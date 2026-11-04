//! OutLayer wiring (PLAN M2.2, M2.2b, M2.2c). Every call uses the fixed verifier source and the
//! contract's own fee, gas and limits (inv. 9, 11), and the LP's own secret (T33). Callback args
//! are built here, never taken from the OutLayer payload (PR #11 review). Callbacks read the raw
//! promise result and never panic on its content: anything unusable is `RunFailed` (D-021).
//!
//! OutLayer `request_execution` (Test A, M0.4; https://app.outlayer.ai/docs/near-integration):
//! `{source, resource_limits, input_data, secrets_ref, response_format:"Json", payer_account_id,
//! params}`; its final value, seen by our `.then` callback, is the program's JSON output, or `null`
//! when the run failed, or nothing on a promise failure / yield timeout.

use crate::verdicts::{VerifierOutput, VerifierResult};
use crate::{Escrow, EscrowExt, Event, OrderState, VerifierSource};
use near_sdk::serde_json::{self, json, Value};
use near_sdk::{env, near, require, AccountId, Gas, GasWeight, NearToken, Promise};

/// Largest program output we read; bigger results are treated as no value (never as output).
const MAX_RESULT_BYTES: usize = 4096;

/// Program output (or `None`): `Some(json)` only for a JSON value that is not `null`.
fn read_output() -> Option<Value> {
    match env::promise_result_checked(0, MAX_RESULT_BYTES) {
        Ok(bytes) if !bytes.is_empty() => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Null) | Err(_) => None,
            Ok(v) => Some(v),
        },
        _ => None,
    }
}

/// Parse a verify output; anything that is not a well-formed `VerifierOutput` is `RunFailed`.
pub fn parse_verify(output: Option<Value>) -> VerifierResult {
    match output.and_then(|v| serde_json::from_value::<VerifierOutput>(v).ok()) {
        Some(o) => VerifierResult::Output(o),
        None => VerifierResult::RunFailed,
    }
}

fn is_key_fp(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[near]
impl Escrow {
    /// Start a verification and ask OutLayer to check the payment (SPEC §4.3). Anyone may call it
    /// (buyer, LP, keeper) and must attach at least the contract's OutLayer fee; any excess is
    /// refunded. If the stuck-timeout path slashes instead (returns 0), the whole deposit is refunded.
    #[payable]
    pub fn claim(&mut self, order_id: u64) -> u32 {
        let fee = self.require_fee();
        let attempt = self.begin_claim(order_id);
        if attempt == 0 {
            refund(env::attached_deposit());
            return 0;
        }
        let order = self.orders.get(&order_id).expect("order exists").clone();
        let grace = self.params.t_grace_ns;
        // D-022: the verifier counts payments up to pay_deadline + t_grace as on time (Unix seconds).
        let deadline_s = order.pay_deadline.0.checked_add(grace).unwrap_or_else(|| env::panic_str("overflow")) / 1_000_000_000;
        let input = json!({
            "mode": "verify",
            "reference": order.reference,
            "expected_amount_pesewas": order.ghs_amount_pesewas.0,
            "currency": "GHS",
            "pay_deadline": deadline_s,
        });
        let secret = self.lps.get(&order.lp).expect("LP exists").secret_ref.clone();
        let cb = Self::ext(env::current_account_id())
            .with_static_gas(Gas::from_tgas(self.params.outlayer_call.callback_gas_tgas))
            .with_unused_gas_weight(0)
            .on_verdict_result(order_id, attempt);
        self.outlayer_request(input, &secret.profile, &secret.account_id, cb).detach();
        refund(env::attached_deposit().saturating_sub(fee));
        attempt
    }

    /// OutLayer callback for `claim`. Never panics on the payload (D-021).
    #[private]
    pub fn on_verdict_result(&mut self, order_id: u64, attempt: u32) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        let result = parse_verify(read_output());
        self.apply_verdict(order_id, attempt, result)
    }

    /// LP: have the TEE fingerprint the key in the LP's OutLayer secret (SPEC §3.1). Only while
    /// `key_fp` is unset and the LP has no open orders. Attach at least the OutLayer fee.
    #[payable]
    pub fn request_fingerprint(&mut self) {
        let fee = self.require_fee();
        let id = env::predecessor_account_id();
        let lp = self.lps.get(&id).unwrap_or_else(|| env::panic_str("not a registered LP")).clone();
        require!(lp.key_fp.is_none(), "key already fingerprinted; use set_secret_ref to change it");
        require!(lp.open_orders == 0, "secret is locked while orders are open");
        let cb = Self::ext(env::current_account_id())
            .with_static_gas(Gas::from_tgas(self.params.outlayer_call.callback_gas_tgas))
            .with_unused_gas_weight(0)
            .on_fingerprint_result(id.clone(), lp.secret_gen);
        self.outlayer_request(json!({"mode": "fingerprint"}), &lp.secret_ref.profile, &id, cb).detach();
        refund(env::attached_deposit().saturating_sub(fee));
    }

    /// OutLayer callback for `request_fingerprint`. Stores `key_fp` only if it is 64 lowercase hex,
    /// the LP still has no `key_fp`, no open orders, and the secret did not change meanwhile
    /// (stale-callback race, PLAN M2.2c). Storage for it was pre-charged at registration.
    #[private]
    pub fn on_fingerprint_result(&mut self, lp_id: AccountId, secret_gen: u32) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        let fp = read_output().and_then(|v| v.get("key_fp").and_then(|f| f.as_str()).map(str::to_string));
        let Some(lp) = self.lps.get_mut(&lp_id) else { return false };
        match fp {
            Some(fp) if is_key_fp(&fp) && lp.key_fp.is_none() && lp.open_orders == 0 && lp.secret_gen == secret_gen => {
                lp.key_fp = Some(fp.clone());
                Event::LpFingerprinted { lp: lp_id, key_fp: fp }.emit();
                true
            }
            _ => false,
        }
    }

    /// Buyer (or anyone): have the TEE create the Paystack checkout for this order with the LP's
    /// own key (SPEC §4.2, T24), GHS and mobile money only. Attach at least the OutLayer fee.
    #[payable]
    pub fn request_checkout(&mut self, order_id: u64) {
        let fee = self.require_fee();
        let order = self.orders.get(&order_id).unwrap_or_else(|| env::panic_str("no such order")).clone();
        require!(order.state == OrderState::AwaitingPayment, "order is not awaiting payment");
        require!(order.checkout_url.is_none(), "checkout already created");
        let secret = self.lps.get(&order.lp).expect("LP exists").secret_ref.clone();
        let input = json!({
            "mode": "checkout",
            "reference": order.reference,
            "amount_pesewas": order.ghs_amount_pesewas.0,
            "currency": "GHS",
        });
        let cb = Self::ext(env::current_account_id())
            .with_static_gas(Gas::from_tgas(self.params.outlayer_call.callback_gas_tgas))
            .with_unused_gas_weight(0)
            .on_checkout_result(order_id);
        self.outlayer_request(input, &secret.profile, &secret.account_id, cb).detach();
        refund(env::attached_deposit().saturating_sub(fee));
    }

    /// OutLayer callback for `request_checkout`: stores the URL if it is a Paystack checkout URL of
    /// at most 256 chars (pre-charged at open) for this order's reference.
    #[private]
    pub fn on_checkout_result(&mut self, order_id: u64) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        let out = read_output();
        let Some(order) = self.orders.get_mut(&order_id) else { return false };
        let url = out.as_ref().and_then(|v| {
            let reference_ok = v.get("reference").and_then(|r| r.as_str()) == Some(order.reference.as_str());
            let url = v.get("authorization_url")?.as_str()?;
            (reference_ok && url.starts_with("https://checkout.paystack.com/") && url.len() <= crate::orders::MAX_CHECKOUT_URL)
                .then(|| url.to_string())
        });
        match url {
            Some(u) if order.state == OrderState::AwaitingPayment && order.checkout_url.is_none() => {
                order.checkout_url = Some(u.clone());
                Event::OrderCheckout { order_id, checkout_url: u }.emit();
                true
            }
            _ => false,
        }
    }
}

impl Escrow {
    fn require_fee(&self) -> NearToken {
        let fee = NearToken::from_yoctonear(self.params.outlayer_call.fee.0);
        require!(env::attached_deposit() >= fee, format!("attach at least {} yoctoNEAR for the OutLayer fee", fee.as_yoctonear()));
        fee
    }

    /// `request_execution` on OutLayer with the fixed verifier, then `callback`.
    fn outlayer_request(&self, input: Value, profile: &str, secret_owner: &AccountId, callback: Promise) -> Promise {
        let c = &self.params.outlayer_call;
        let source = match &self.verifier {
            VerifierSource::WasmUrl { url, hash } => json!({"WasmUrl": {"url": url, "hash": hash, "build_target": "wasm32-wasip2"}}),
            VerifierSource::Project { project_id, version_key } => {
                json!({"Project": {"project_id": project_id, "version_key": version_key}})
            }
        };
        let args = json!({
            "source": source,
            "resource_limits": {
                "max_instructions": c.max_instructions,
                "max_memory_mb": c.max_memory_mb,
                "max_execution_seconds": c.max_execution_seconds,
            },
            "input_data": input.to_string(),
            "secrets_ref": {"profile": profile, "account_id": secret_owner},
            "response_format": "Json",
            "payer_account_id": null,
            "params": null,
        });
        Promise::new(self.outlayer.clone())
            .function_call_weight(
                "request_execution".to_string(),
                args.to_string().into_bytes(),
                NearToken::from_yoctonear(c.fee.0),
                Gas::from_tgas(c.gas_tgas),
                GasWeight(0),
            )
            .then(callback)
    }
}

fn refund(amount: NearToken) {
    if !amount.is_zero() {
        Promise::new(env::predecessor_account_id()).transfer(amount).detach();
    }
}
