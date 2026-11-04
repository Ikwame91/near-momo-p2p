//! M1.6 tests: slash windows (LP_FAULT `t_retry` / outage `t_outage_max`), min failed attempts,
//! accounting (bond pays the buyer, inv. 5 and 6), automatic slash inside the verdict step, the
//! public `slash` backstop and the stuck-timeout path.

use crate::tests::{acc, init_with, params, verifier, ME};
use crate::verdicts::{slash_due, VERIFY_TIMEOUT_NS};
use crate::{Escrow, OrderState, Params, Quote, SecretRef, Verdict, VerifierOutput, VerifierResult};
use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, json, Value};
use near_sdk::test_utils::{get_created_receipts, get_logs, VMContextBuilder};
use near_sdk::{testing_env, NearToken};

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp.testnet";
const BUYER: &str = "ama.testnet";
const USDC: u128 = 1_000_000;
const MIN: u64 = 60_000_000_000;
const T0: u64 = 1_800_000_000_000_000_000;
const T_RETRY: u64 = 3 * MIN; // demo values (SPEC §7)
const T_OUTAGE: u64 = 6 * MIN;

fn at(who: &str, time: u64) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(NearToken::from_millinear(if who == ME || who == TOKEN { 0 } else { 100 }))
        .block_timestamp(time)
        .build());
}

fn fp() -> String {
    "ab".repeat(32)
}

/// LP with bond 20 and liquidity 300; one 10 USDC order (id 0) from a new buyer.
fn setup(p: Params) -> Escrow {
    at(ME, T0);
    let mut e = init_with(verifier(), p);
    at(LP, T0);
    e.register_lp(
        "paystack".into(),
        SecretRef { profile: "sec".into(), account_id: acc(LP) },
        Quote {
            rate_pesewas_per_usdc: U128(1_500),
            min_usdc: U128(USDC),
            max_usdc: U128(200 * USDC),
            new_buyer_exposure_max: U128(100 * USDC),
        },
    );
    at(TOKEN, T0);
    e.ft_on_transfer(acc(LP), U128(20 * USDC), "bond".into()).detach();
    e.ft_on_transfer(acc(LP), U128(300 * USDC), "liquidity".into()).detach();
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some(fp());
    e.lps.flush();
    at(BUYER, T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    e
}

fn demo() -> Escrow {
    setup(params())
}

fn reference(e: &Escrow, id: u64) -> String {
    e.get_order(id).unwrap().reference
}

fn unverifiable(e: &Escrow, id: u64, reason: &str) -> VerifierResult {
    VerifierResult::Output(VerifierOutput {
        verdict: Verdict::Unverifiable,
        reference: reference(e, id),
        key_fp: fp(),
        reason: reason.into(),
    })
}

/// Claim order `id` at `time` and deliver `result` for that attempt.
fn fail(e: &mut Escrow, id: u64, time: u64, result: VerifierResult) {
    at("anyone.testnet", time);
    let attempt = e.claim(id);
    at(ME, time);
    e.on_verdict(id, attempt, result);
}

fn lp_fault(e: &mut Escrow, time: u64) {
    let r = unverifiable(e, 0, "auth_error");
    fail(e, 0, time, r);
}

fn outage(e: &mut Escrow, time: u64) {
    fail(e, 0, time, VerifierResult::RunFailed);
}

fn state(e: &Escrow) -> OrderState {
    e.get_order(0).unwrap().state
}

fn last_event() -> (String, Value) {
    let l = get_logs().into_iter().rev().find(|l| l.starts_with("EVENT_JSON:")).expect("an event");
    let v: Value = serde_json::from_str(l.strip_prefix("EVENT_JSON:").unwrap()).unwrap();
    (v["event"].as_str().unwrap().to_string(), v["data"].clone())
}

// ---- windows ----

#[test]
fn lp_fault_streak_slashes_at_exactly_t_retry() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Slashed);
}

#[test]
fn lp_fault_streak_one_ns_short_of_t_retry_does_not_slash() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY - 1);
    assert_eq!(state(&e), OrderState::Unverified);
}

#[test]
fn outage_streak_needs_the_longer_window() {
    // D-012: an honest LP gets t_outage_max during an outage, not t_retry.
    let mut e = demo();
    outage(&mut e, T0 + MIN);
    outage(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Unverified, "t_retry is not enough for outages");
    outage(&mut e, T0 + MIN + T_OUTAGE - 1);
    assert_eq!(state(&e), OrderState::Unverified);
    outage(&mut e, T0 + MIN + T_OUTAGE);
    assert_eq!(state(&e), OrderState::Slashed, "every failure ends in a slash at the hard limit");
}

#[test]
fn an_lp_fault_in_an_outage_streak_switches_to_the_short_window() {
    let mut e = demo();
    outage(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Slashed);
}

#[test]
fn not_paid_in_the_middle_resets_the_streak() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    let r = VerifierResult::Output(VerifierOutput {
        verdict: Verdict::NotPaid,
        reference: reference(&e, 0),
        key_fp: fp(),
        reason: "not_found".into(),
    });
    fail(&mut e, 0, T0 + 2 * MIN, r);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures), (OrderState::Unverified, 1));
}

#[test]
fn min_failed_attempts_is_enforced() {
    let mut p = params();
    p.min_failed_attempts = 3;
    let mut e = setup(p);
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY); // window met, but only 2 failures
    assert_eq!(state(&e), OrderState::Unverified);
    lp_fault(&mut e, T0 + MIN + T_RETRY + 1);
    assert_eq!(state(&e), OrderState::Slashed);
}

// ---- accounting (inv. 5, 6) ----

#[test]
fn slash_pays_the_buyer_from_the_bond_and_frees_the_liquidity() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!(
        (lp.bond.0, lp.liquidity.0, lp.locked.0, lp.open_orders, lp.new_buyer_open.0),
        (10 * USDC, 300 * USDC, 0, 0, 0)
    );
    assert_eq!(e.get_claimable(acc(BUYER)).0, 10 * USDC);
    assert!(get_created_receipts().is_empty(), "no promise from the verdict path (D-021)");
    assert_eq!(
        last_event(),
        ("order_slashed".into(), json!({"order_id": 0, "lp": LP, "buyer": BUYER, "amount": "10000000", "lp_fault": true}))
    );
}

#[test]
fn bond_rule_still_holds_after_a_slash_with_other_orders_open() {
    // bond 20: order 0 (10) + order 1 (10) = 20 locked; slash order 0 → bond 10, locked 10.
    let mut e = demo();
    at("kofi.testnet", T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.bond.0, lp.locked.0, lp.open_orders), (10 * USDC, 10 * USDC, 1));
    assert!(lp.locked.0 <= lp.bond.0, "inv. 5");
}

#[test]
#[should_panic(expected = "bond is locked while orders are open")]
fn bond_stays_locked_while_the_other_order_is_open() {
    let mut e = demo();
    at("kofi.testnet", T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    at(LP, T0 + MIN + T_RETRY);
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(LP))
        .attached_deposit(NearToken::from_yoctonear(1))
        .build());
    let _ = e.withdraw_bond(U128(USDC));
}

#[test]
fn slashed_buyer_is_paid_through_withdraw_claimable() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    at("anyone.testnet", T0 + MIN + T_RETRY);
    let _ = e.withdraw_claimable(acc(BUYER));
    assert_eq!(e.get_claimable(acc(BUYER)).0, 0);
    assert_eq!(get_created_receipts().len(), 2, "ft_transfer + on_payout");
}

#[test]
fn slash_does_not_count_as_a_released_order() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(e.get_buyer_released(acc(LP), acc(BUYER)), 0);
}

// ---- terminal ----

#[test]
#[should_panic(expected = "order cannot be claimed in its current state")]
fn a_slashed_order_cannot_be_claimed() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    e.claim(0);
}

#[test]
fn a_late_paid_verdict_after_the_slash_is_ignored() {
    // The LP cannot undo the slash with a late answer (stale attempt, terminal state).
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    at("anyone.testnet", T0 + MIN + T_RETRY);
    let attempt = e.claim(0);
    at(ME, T0 + MIN + T_RETRY);
    let r = unverifiable(&e, 0, "auth_error");
    e.on_verdict(0, attempt, r);
    let paid = VerifierResult::Output(VerifierOutput {
        verdict: Verdict::Paid,
        reference: reference(&e, 0),
        key_fp: fp(),
        reason: "ok".into(),
    });
    assert!(!e.on_verdict(0, attempt, paid));
    assert_eq!(state(&e), OrderState::Slashed);
    assert_eq!(e.get_claimable(acc(BUYER)).0, 10 * USDC, "paid once, from the bond");
}

// ---- public backstop ----

#[test]
#[should_panic(expected = "slash conditions not met")]
fn public_slash_before_the_window_fails() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    e.slash(0);
}

#[test]
#[should_panic(expected = "slash conditions not met")]
fn public_slash_on_an_awaiting_order_fails() {
    let mut e = demo();
    e.slash(0);
}

#[test]
#[should_panic(expected = "no such order")]
fn public_slash_of_unknown_order_fails() {
    let mut e = demo();
    e.slash(5);
}

// ---- stuck VERIFYING path ----

#[test]
fn a_missing_answer_can_complete_the_slash_window() {
    // First failure at T1; then a claim whose answer never comes. The timeout re-claim records an
    // outage failure 6+ min later, which meets the outage window: slash instead of re-verifying.
    let mut e = demo();
    outage(&mut e, T0 + MIN);
    at("anyone.testnet", T0 + 2 * MIN);
    e.claim(0); // never answered
    at("anyone.testnet", T0 + 2 * MIN + VERIFY_TIMEOUT_NS + 1);
    assert_eq!(e.claim(0), 0, "0 = slashed instead of a new attempt");
    assert_eq!(state(&e), OrderState::Slashed);
}

// ---- the pure rule ----

#[test]
fn slash_due_requires_unverified_state() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    let mut o = e.get_order(0).unwrap();
    o.failures = 9;
    o.last_failure_at = Some(near_sdk::json_types::U64(T0 + 100 * MIN));
    assert!(slash_due(&o, 2, T_RETRY, T_OUTAGE));
    o.state = OrderState::Verifying;
    assert!(!slash_due(&o, 2, T_RETRY, T_OUTAGE));
    o.state = OrderState::Released;
    assert!(!slash_due(&o, 2, T_RETRY, T_OUTAGE));
}
