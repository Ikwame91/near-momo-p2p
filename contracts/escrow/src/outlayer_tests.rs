//! M2.2 tests: OutLayer calls (fee, exact request_execution args, callbacks) and payload parsing.
// `testing_env_with_promise_results` is deprecated in favour of the 5-argument `testing_env!`,
// which needs the `near_parameters` crate as a dev-dependency; the helper does the same job.
#![allow(deprecated)]

use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, OrderState, Quote, SecretRef};
use near_sdk::json_types::U128;
use near_sdk::mock::MockAction;
use near_sdk::serde_json::{self, json, Value};
use near_sdk::test_utils::{get_created_receipts, testing_env_with_promise_results, VMContextBuilder};
use near_sdk::{testing_env, Gas, NearToken, PromiseResult};

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp.testnet";
const BUYER: &str = "ama.testnet";
const USDC: u128 = 1_000_000;
const T0: u64 = 1_800_000_000_000_000_000;
const FEE: u128 = 10_000_000_000_000_000_000_000; // params().outlayer_call.fee (0.01 NEAR)

fn ctx(who: &str, yocto: u128) -> near_sdk::VMContext {
    VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(NearToken::from_yoctonear(yocto))
        .block_timestamp(T0)
        .build()
}

fn at(who: &str, yocto: u128) {
    testing_env!(ctx(who, yocto));
}

/// Callback context: the contract calls itself with OutLayer's result as promise result 0.
fn callback(result: PromiseResult) {
    testing_env_with_promise_results(ctx(ME, 0), result);
}

fn ok(v: Value) -> PromiseResult {
    PromiseResult::Successful(v.to_string().into_bytes())
}

fn fp() -> String {
    "ab".repeat(32)
}

/// Registered paystack LP with bond/liquidity, NOT yet fingerprinted.
fn registered() -> Escrow {
    at(ME, 0);
    let mut e = init_with(verifier(), params());
    at(LP, 100_000_000_000_000_000_000_000);
    e.register_lp(
        "paystack".into(),
        SecretRef { profile: "paystack".into(), account_id: acc(LP) },
        Quote {
            rate_pesewas_per_usdc: U128(1_500),
            min_usdc: U128(USDC),
            max_usdc: U128(200 * USDC),
            new_buyer_exposure_max: U128(100 * USDC),
        },
    );
    at(TOKEN, 0);
    e.ft_on_transfer(acc(LP), U128(200 * USDC), "bond".into()).detach();
    e.ft_on_transfer(acc(LP), U128(300 * USDC), "liquidity".into()).detach();
    e
}

fn with_order() -> Escrow {
    let mut e = registered();
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some(fp());
    e.lps.flush();
    at(BUYER, 100_000_000_000_000_000_000_000);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    e
}

/// (receiver, method, args, deposit, gas) of every function call created.
fn calls() -> Vec<(String, String, Value, NearToken, Gas)> {
    let mut out = vec![];
    for r in get_created_receipts() {
        for a in r.actions {
            if let MockAction::FunctionCallWeight { method_name, args, attached_deposit, prepaid_gas, .. } = a {
                out.push((
                    r.receiver_id.to_string(),
                    String::from_utf8(method_name).unwrap(),
                    serde_json::from_slice(&args).unwrap_or(Value::Null),
                    attached_deposit,
                    prepaid_gas,
                ));
            }
        }
    }
    out
}

fn refunds_to(who: &str) -> Vec<NearToken> {
    get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id.as_str() == who)
        .flat_map(|r| r.actions)
        .filter_map(|a| if let MockAction::Transfer { deposit, .. } = a { Some(deposit) } else { None })
        .collect()
}

// ---- claim → request_execution ----

#[test]
#[should_panic(expected = "for the OutLayer fee")]
fn claim_without_the_fee_fails() {
    let mut e = with_order();
    at("anyone.testnet", FEE - 1);
    e.claim(0);
}

#[test]
fn claim_sends_the_exact_verify_request_and_refunds_the_excess() {
    let mut e = with_order();
    at("anyone.testnet", FEE + 5);
    assert_eq!(e.claim(0), 1);
    let c = calls();
    let (to, m, args, dep, gas) = &c[0];
    assert_eq!((to.as_str(), m.as_str()), ("outlayer.testnet", "request_execution"));
    assert_eq!((*dep, *gas), (NearToken::from_yoctonear(FEE), Gas::from_tgas(200)));
    let hash = "021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61";
    assert_eq!(args["source"]["WasmUrl"]["hash"], hash, "the fixed verifier (inv. 9)");
    assert_eq!(args["source"]["WasmUrl"]["build_target"], "wasm32-wasip2");
    assert_eq!(args["resource_limits"], json!({"max_instructions": 1_000_000_000u64, "max_memory_mb": 128, "max_execution_seconds": 20}));
    assert_eq!(args["secrets_ref"], json!({"profile": "paystack", "account_id": LP}), "the LP's own secret");
    assert_eq!(args["response_format"], "Json");
    let input: Value = serde_json::from_str(args["input_data"].as_str().unwrap()).unwrap();
    let order = e.get_order(0).unwrap();
    let deadline_s = (order.pay_deadline.0 + 2 * 60_000_000_000) / 1_000_000_000; // + t_grace (D-022)
    assert_eq!(
        input,
        json!({"mode": "verify", "reference": order.reference, "expected_amount_pesewas": 15000,
               "currency": "GHS", "pay_deadline": deadline_s})
    );
    let (to, m, args, _, gas) = &c[1];
    assert_eq!((to.as_str(), m.as_str()), (ME, "on_verdict_result"));
    assert_eq!(args, &json!({"order_id": 0, "attempt": 1}), "callback args come from the contract");
    assert_eq!(*gas, Gas::from_tgas(30));
    assert_eq!(refunds_to("anyone.testnet"), vec![NearToken::from_yoctonear(5)]);
}

// ---- on_verdict_result: payload parsing never panics ----

fn claimed() -> Escrow {
    let mut e = with_order();
    at("anyone.testnet", FEE);
    e.claim(0);
    e
}

fn verify_output(e: &Escrow, verdict: &str, reason: &str) -> Value {
    json!({"verdict": verdict, "reference": e.get_order(0).unwrap().reference, "key_fp": fp(), "reason": reason})
}

#[test]
fn paid_payload_releases() {
    let mut e = claimed();
    let v = verify_output(&e, "PAID", "ok");
    callback(ok(v));
    assert!(e.on_verdict_result(0, 1));
    assert_eq!(e.get_order(0).unwrap().state, OrderState::Released);
    assert_eq!(e.get_claimable(acc(BUYER)).0, 10 * USDC);
}

#[test]
fn not_paid_and_pending_payloads_are_handled() {
    let mut e = claimed();
    let v = verify_output(&e, "PENDING", "pending");
    callback(ok(v));
    e.on_verdict_result(0, 1);
    assert_eq!(e.get_order(0).unwrap().state, OrderState::Unverified);
}

#[test]
fn null_garbage_failed_and_unknown_payloads_are_run_failed() {
    // One escrow, one order per case (mocked storage persists within a test).
    let cases: Vec<PromiseResult> = vec![
        ok(Value::Null),
        PromiseResult::Successful(b"not json".to_vec()),
        PromiseResult::Successful(vec![]),
        PromiseResult::Failed,
        ok(json!({"verdict": "MAYBE", "reference": "x", "key_fp": "y", "reason": "z"})),
        ok(json!({"key_fp": "only"})),
        PromiseResult::Successful(vec![b'1'; 5000]), // over MAX_RESULT_BYTES
    ];
    let mut e = with_order();
    for (i, r) in cases.into_iter().enumerate() {
        let buyer = format!("b{i}.testnet");
        at(&buyer, 100_000_000_000_000_000_000_000);
        let id = e.open_buy_order(acc(LP), U128(USDC), None).id;
        at("anyone.testnet", FEE);
        let attempt = e.claim(id);
        callback(r);
        assert!(e.on_verdict_result(id, attempt), "case {i}");
        let o = e.get_order(id).unwrap();
        assert_eq!((o.state, o.failures, o.lp_fault), (OrderState::Unverified, 1, false), "case {i}: outage");
        assert_eq!(e.get_claimable(acc(&buyer)).0, 0, "case {i}: no money moves");
    }
}

#[test]
#[should_panic(expected = "only the contract itself")]
fn verdict_callback_from_a_stranger_is_rejected() {
    let mut e = claimed();
    testing_env_with_promise_results(ctx("attacker.testnet", 0), ok(Value::Null));
    e.on_verdict_result(0, 1);
}

// ---- fingerprint ----

#[test]
fn request_fingerprint_sends_fingerprint_mode_for_the_lps_secret() {
    let mut e = registered();
    at(LP, FEE);
    e.request_fingerprint();
    let c = calls();
    let input: Value = serde_json::from_str(c[0].2["input_data"].as_str().unwrap()).unwrap();
    assert_eq!(input, json!({"mode": "fingerprint"}));
    assert_eq!(c[0].2["secrets_ref"], json!({"profile": "paystack", "account_id": LP}));
    assert_eq!((c[1].1.as_str(), &c[1].2), ("on_fingerprint_result", &json!({"lp_id": LP, "secret_gen": 0})));
}

#[test]
fn fingerprint_result_is_stored_and_lets_the_lp_take_orders() {
    let mut e = registered();
    callback(ok(json!({"key_fp": fp()})));
    assert!(e.on_fingerprint_result(acc(LP), 0));
    assert_eq!(e.get_lp(acc(LP)).unwrap().key_fp, Some(fp()));
    at(BUYER, 100_000_000_000_000_000_000_000);
    e.open_buy_order(acc(LP), U128(USDC), None);
}

#[test]
fn bad_or_stale_fingerprints_are_ignored() {
    let mut e = registered();
    callback(ok(json!({"key_fp": "XYZ"})));
    assert!(!e.on_fingerprint_result(acc(LP), 0), "not 64 hex");
    callback(ok(json!({"error": "no_key"})));
    assert!(!e.on_fingerprint_result(acc(LP), 0), "no key");
    callback(ok(json!({"key_fp": fp()})));
    assert!(!e.on_fingerprint_result(acc(LP), 7), "stale secret_gen (secret changed meanwhile)");
    assert_eq!(e.get_lp(acc(LP)).unwrap().key_fp, None);
}

#[test]
fn set_secret_ref_bumps_secret_gen() {
    let mut e = registered();
    at(LP, 1);
    e.set_secret_ref(SecretRef { profile: "paystack-v2".into(), account_id: acc(LP) });
    assert_eq!(e.get_lp(acc(LP)).unwrap().secret_gen, 1);
    callback(ok(json!({"key_fp": fp()})));
    assert!(!e.on_fingerprint_result(acc(LP), 0), "an answer for the old secret is ignored");
}

#[test]
#[should_panic(expected = "key already fingerprinted")]
fn request_fingerprint_twice_fails() {
    let mut e = with_order();
    at(LP, FEE);
    e.request_fingerprint();
}

// ---- checkout ----

#[test]
fn checkout_url_is_stored_for_this_order() {
    let mut e = with_order();
    at(BUYER, FEE);
    e.request_checkout(0);
    let input: Value = serde_json::from_str(calls()[0].2["input_data"].as_str().unwrap()).unwrap();
    let r = e.get_order(0).unwrap().reference;
    assert_eq!(input, json!({"mode": "checkout", "reference": r, "amount_pesewas": 15000, "currency": "GHS"}));
    callback(ok(json!({"authorization_url": "https://checkout.paystack.com/abc123", "reference": r, "key_fp": fp()})));
    assert!(e.on_checkout_result(0));
    assert_eq!(e.get_order(0).unwrap().checkout_url.as_deref(), Some("https://checkout.paystack.com/abc123"));
}

#[test]
fn foreign_or_mismatched_checkout_urls_are_ignored() {
    let mut e = with_order();
    let r = e.get_order(0).unwrap().reference;
    callback(ok(json!({"authorization_url": "https://evil.example/pay", "reference": r})));
    assert!(!e.on_checkout_result(0));
    callback(ok(json!({"authorization_url": "https://checkout.paystack.com/abc", "reference": "opr-00000000-9"})));
    assert!(!e.on_checkout_result(0));
    callback(ok(json!({"error": "auth_error", "reference": r})));
    assert!(!e.on_checkout_result(0));
    assert_eq!(e.get_order(0).unwrap().checkout_url, None);
}

#[test]
#[should_panic(expected = "checkout already created")]
fn second_checkout_fails() {
    let mut e = with_order();
    let r = e.get_order(0).unwrap().reference;
    callback(ok(json!({"authorization_url": "https://checkout.paystack.com/abc", "reference": r})));
    e.on_checkout_result(0);
    at(BUYER, FEE);
    e.request_checkout(0);
}
