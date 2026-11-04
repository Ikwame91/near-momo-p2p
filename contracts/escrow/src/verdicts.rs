//! Verdict handling (PLAN M1.5, SPEC §4.3–4.5, §6). The OutLayer call itself arrives in M2.2;
//! here the verifier's result is a typed input to the `on_verdict` callback ("mock verifier"),
//! which is exactly what M2.2 will feed it after parsing the OutLayer response.
//!
//! Rules: only a verdict for the order's CURRENT attempt, while VERIFYING, counts (T15); it must
//! carry this order's reference and the LP's registered key_fp, otherwise it is UNVERIFIABLE
//! (inv. 10). USDC is released only on PAID (inv. 1). PENDING / UNVERIFIABLE / a failed run never
//! count as NOT_PAID (inv. 2); they mark the order UNVERIFIED and count a failure for M1.6.

use crate::lp::{FT_TRANSFER_MIN_GAS, WITHDRAW_CALLBACK_GAS};
use crate::{Escrow, EscrowExt, Event, OrderState};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::json;
use near_sdk::{env, near, require, AccountId, GasWeight, NearToken, Promise, PromiseError};

#[near(serializers = [json])]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Paid,
    NotPaid,
    Pending,
    Unverifiable,
}

/// The verifier program's output (SPEC §4.4): `{verdict, reference, key_fp, reason}`.
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub struct VerifierOutput {
    pub verdict: Verdict,
    pub reference: String,
    pub key_fp: String,
    pub reason: String,
}

/// What reached the callback: the program's output, or nothing usable (OutLayer `null`, no value,
/// not JSON, yield timeout). The latter is always `OUTAGE` (D-012).
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub enum VerifierResult {
    Output(VerifierOutput),
    RunFailed,
}

/// Verifier reasons that are the LP's fault (D-012, D-017): key rejected (HTTP 401) or missing.
/// Every other reason, and anything unknown, is an outage.
const LP_FAULT_REASONS: [&str; 2] = ["auth_error", "no_key"];

#[near]
impl Escrow {
    /// Start a verification (SPEC §4.3). Anyone may call it (the buyer, the LP, a keeper):
    /// AWAITING_PAYMENT until `pay_deadline + t_grace` (inclusive; `expire_order` is strictly
    /// after), or UNVERIFIED at any time (a re-check). Returns the new attempt number, which the
    /// verdict must carry. M2.2 adds the OutLayer call and the contract-set fee.
    pub fn claim(&mut self, order_id: u64) -> u32 {
        let now = env::block_timestamp();
        let grace = self.params.t_grace_ns;
        let order = self.orders.get_mut(&order_id).unwrap_or_else(|| env::panic_str("no such order"));
        match order.state {
            OrderState::AwaitingPayment => {
                let last = order.pay_deadline.0.checked_add(grace).unwrap_or_else(|| env::panic_str("overflow"));
                require!(now <= last, "claim window has passed; the order can be expired");
            }
            OrderState::Unverified => {}
            _ => env::panic_str("order cannot be claimed in its current state"),
        }
        order.state = OrderState::Verifying;
        order.attempt = order.attempt.checked_add(1).unwrap_or_else(|| env::panic_str("overflow"));
        let attempt = order.attempt;
        Event::OrderVerifying { order_id, attempt }.emit();
        attempt
    }

    /// Verdict callback (from the OutLayer resume in M2.2). Returns false if the verdict was
    /// ignored as stale. The caller check is explicit (D-016).
    #[private]
    pub fn on_verdict(&mut self, order_id: u64, attempt: u32, result: VerifierResult) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        let now = env::block_timestamp();
        let (grace, delay) = (self.params.t_grace_ns, self.params.release_delay_ns);
        let Some(order) = self.orders.get(&order_id) else { return false };
        if order.state != OrderState::Verifying || order.attempt != attempt {
            return false; // a late or duplicate callback for an older request (T15)
        }
        let (reference, lp_id) = (order.reference.clone(), order.lp.clone());
        let key_fp = self.lps.get(&lp_id).and_then(|l| l.key_fp.clone());

        // Decide the outcome; anything that does not match this order is UNVERIFIABLE (inv. 10).
        let outcome = match result {
            VerifierResult::RunFailed => Outcome::Failure { lp_fault: false },
            VerifierResult::Output(o) => {
                if key_fp.as_deref() != Some(o.key_fp.as_str()) {
                    Outcome::Failure { lp_fault: true } // different key than registered (T24)
                } else if o.reference != reference {
                    Outcome::Failure { lp_fault: false }
                } else {
                    match o.verdict {
                        Verdict::Paid => Outcome::Paid,
                        Verdict::NotPaid => Outcome::NotPaid,
                        Verdict::Pending => Outcome::Failure { lp_fault: false },
                        Verdict::Unverifiable => {
                            Outcome::Failure { lp_fault: LP_FAULT_REASONS.contains(&o.reason.as_str()) }
                        }
                    }
                }
            }
        };

        let order = self.orders.get_mut(&order_id).expect("checked above");
        match outcome {
            Outcome::Paid => {
                if order.new_buyer && delay > 0 {
                    let at = now.checked_add(delay).unwrap_or_else(|| env::panic_str("overflow"));
                    order.state = OrderState::ReleasePending;
                    order.release_at = Some(U64(at));
                    Event::OrderReleasePending { order_id, release_at: U64(at) }.emit();
                } else {
                    self.release(order_id);
                }
            }
            Outcome::NotPaid => {
                let last = order.pay_deadline.0.checked_add(grace).unwrap_or_else(|| env::panic_str("overflow"));
                // A clear answer ends the unverified streak.
                order.failures = 0;
                order.first_failure_at = None;
                order.last_failure_at = None;
                order.lp_fault = false;
                if now <= last {
                    order.state = OrderState::AwaitingPayment;
                    Event::OrderNotPaid { order_id, expired: false }.emit();
                } else {
                    order.state = OrderState::Expired;
                    let (amount, new_buyer) = (order.usdc_amount.0, order.new_buyer);
                    self.unlock(&lp_id, amount, true, new_buyer);
                    Event::OrderNotPaid { order_id, expired: true }.emit();
                }
            }
            Outcome::Failure { lp_fault } => {
                order.state = OrderState::Unverified;
                order.failures = order.failures.saturating_add(1);
                if order.first_failure_at.is_none() {
                    order.first_failure_at = Some(U64(now));
                }
                order.last_failure_at = Some(U64(now));
                order.lp_fault |= lp_fault;
                Event::OrderUnverified { order_id, attempt, failures: order.failures, lp_fault: order.lp_fault }.emit();
            }
        }
        true
    }

    /// Release a RELEASE_PENDING order once `release_at` has passed. Anyone may call it.
    pub fn finalize_release(&mut self, order_id: u64) {
        let order = self.orders.get(&order_id).unwrap_or_else(|| env::panic_str("no such order"));
        require!(order.state == OrderState::ReleasePending, "order is not pending release");
        let at = order.release_at.expect("set with ReleasePending").0;
        require!(env::block_timestamp() >= at, "release delay has not passed");
        self.release(order_id);
    }

    /// Pay out an account's claimable balance (credited when a payout transfer failed, e.g. the
    /// account was not registered on the token). Anyone may trigger it; it always pays `account`.
    pub fn withdraw_claimable(&mut self, account: AccountId) -> Promise {
        let amount = self.claimable.remove(&account).unwrap_or(0);
        require!(amount > 0, "nothing claimable");
        self.pay_out(account, amount)
    }

    pub fn get_claimable(&self, account: AccountId) -> U128 {
        U128(self.claimable.get(&account).copied().unwrap_or(0))
    }

    /// Callback after a payout transfer: credit the claimable balance if it failed.
    #[private]
    pub fn on_payout(&mut self, account: AccountId, amount: U128, #[callback_result] result: Result<(), PromiseError>) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        if result.is_ok() {
            return true;
        }
        let cur = self.claimable.get(&account).copied().unwrap_or(0);
        self.claimable.insert(account.clone(), cur.checked_add(amount.0).unwrap_or_else(|| env::panic_str("overflow")));
        Event::ClaimableCredited { account, amount }.emit();
        false
    }
}

enum Outcome {
    Paid,
    NotPaid,
    Failure { lp_fault: bool },
}

impl Escrow {
    /// Final release of a buy order: RELEASED, the LP's lock closes (the USDC leaves the LP), the
    /// buyer's history with this LP grows (D-020), and the USDC is sent to the buyer.
    fn release(&mut self, order_id: u64) {
        let order = self.orders.get_mut(&order_id).expect("order exists");
        order.state = OrderState::Released;
        order.release_at = None;
        let (lp, buyer, amount, new_buyer) = (order.lp.clone(), order.buyer.clone(), order.usdc_amount.0, order.new_buyer);
        self.unlock(&lp, amount, false, new_buyer);
        let key = (lp, buyer.clone());
        let n = self.buyer_released.get(&key).copied().unwrap_or(0);
        self.buyer_released.insert(key, n.checked_add(1).unwrap_or_else(|| env::panic_str("overflow")));
        Event::OrderReleased { order_id, buyer: buyer.clone(), amount: U128(amount) }.emit();
        self.pay_out(buyer, amount).detach();
    }

    /// Send `amount` USDC to `account`; on failure `on_payout` credits a claimable balance, so the
    /// books stay exact (inv. 6). Same gas pattern as withdrawals (D-016).
    pub(crate) fn pay_out(&mut self, account: AccountId, amount: u128) -> Promise {
        Promise::new(self.token.clone())
            .function_call_weight(
                "ft_transfer".to_string(),
                json!({ "receiver_id": account, "amount": U128(amount), "memo": "openramp payout" })
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(1),
                FT_TRANSFER_MIN_GAS,
                GasWeight(1),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(WITHDRAW_CALLBACK_GAS)
                    .with_unused_gas_weight(0)
                    .on_payout(account, U128(amount)),
            )
    }
}
