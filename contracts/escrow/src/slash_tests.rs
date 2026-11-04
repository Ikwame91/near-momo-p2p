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
    let attempt = e.begin_claim(id);
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
fn an_lp_fault_after_outages_gets_t_retry_from_its_first_fault() {
    // PR #12 review / D-022: the short window counts from the first LP_FAULT, not from an older
    // outage failure, so an LP's first fault never slashes instantly.
    let mut e = demo();
    outage(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY); // first LP fault at T0 + 4 min
    assert_eq!(state(&e), OrderState::Unverified);
    lp_fault(&mut e, T0 + MIN + 2 * T_RETRY - 1);
    assert_eq!(state(&e), OrderState::Unverified);
    lp_fault(&mut e, T0 + MIN + 2 * T_RETRY); // t_retry after the first LP fault
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
    e.begin_claim(0);
}

#[test]
fn a_late_paid_verdict_after_the_slash_is_ignored() {
    // The LP cannot undo the slash with a late answer (stale attempt, terminal state).
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    at("anyone.testnet", T0 + MIN + T_RETRY);
    let attempt = e.begin_claim(0);
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
fn a_missing_answer_for_a_check_started_before_the_window_does_not_slash() {
    // PR #12 review: the stuck check was started before the window was met, so it cannot complete
    // it; the missing answer is stamped when it was due and a fresh check is started instead.
    let mut e = demo();
    outage(&mut e, T0 + MIN);
    at("anyone.testnet", T0 + 2 * MIN);
    e.begin_claim(0); // never answered
    at("anyone.testnet", T0 + 2 * MIN + VERIFY_TIMEOUT_NS + 1);
    let attempt = e.begin_claim(0);
    assert!(attempt > 0, "a fresh verification, not a slash");
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures), (OrderState::Verifying, 2));
    assert_eq!(o.last_failure_at.unwrap().0, T0 + 2 * MIN + VERIFY_TIMEOUT_NS, "stamped when due");
    let names: Vec<String> = get_logs().iter().filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .map(|j| serde_json::from_str::<Value>(j).unwrap()["event"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, vec!["order_unverified", "order_verifying"]);
}

#[test]
fn a_missing_answer_for_a_check_started_after_the_window_slashes() {
    let mut e = demo();
    outage(&mut e, T0 + MIN);
    at("anyone.testnet", T0 + MIN + T_OUTAGE);
    e.begin_claim(0); // started after the outage window was met; never answered
    at("anyone.testnet", T0 + MIN + T_OUTAGE + VERIFY_TIMEOUT_NS + 1);
    assert_eq!(e.begin_claim(0), 0, "0 = slashed instead of a new attempt");
    assert_eq!(state(&e), OrderState::Slashed);
    let names: Vec<String> = get_logs().iter().filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .map(|j| serde_json::from_str::<Value>(j).unwrap()["event"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, vec!["order_unverified", "order_slashed"]);
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

// ---- edge cases from the PR #12 code review ----

fn pause(e: &mut Escrow) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(LP))
        .attached_deposit(NearToken::from_yoctonear(1))
        .block_timestamp(T0)
        .build());
    e.pause_lp();
}

#[test]
fn edge_experienced_buyer_slash_keeps_other_new_buyer_exposure() {
    let mut e = demo(); // order 0: BUYER, new
    // order 1: an experienced buyer
    e.buyer_released.insert((acc(LP), acc("exp.testnet")), 3);
    at("exp.testnet", T0);
    let o = e.open_buy_order(acc(LP), U128(10 * USDC), None);
    assert!(!o.new_buyer);
    assert_eq!(e.get_lp(acc(LP)).unwrap().new_buyer_open.0, 10 * USDC);
    let r = unverifiable(&e, 1, "auth_error");
    fail(&mut e, 1, T0 + MIN, r);
    let r = unverifiable(&e, 1, "auth_error");
    fail(&mut e, 1, T0 + MIN + T_RETRY, r);
    assert_eq!(e.get_order(1).unwrap().state, OrderState::Slashed);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.new_buyer_open.0, lp.locked.0, lp.bond.0), (10 * USDC, 10 * USDC, 10 * USDC));
}

#[test]
fn edge_slash_while_paused() {
    let mut e = demo();
    pause(&mut e);
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Slashed);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert!(lp.paused);
    assert_eq!((lp.bond.0, lp.open_orders), (10 * USDC, 0));
}

#[test]
fn edge_failures_saturate_at_255_and_still_slash() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    e.orders.get_mut(&0).unwrap().failures = 255;
    lp_fault(&mut e, T0 + MIN + 1);
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures), (OrderState::Unverified, 255));
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Slashed);
}

#[test]
fn edge_slash_after_claim_until_extension() {
    let mut e = demo();
    // claim before the deadline, NotPaid answered after deadline + grace: claim_until extended.
    at("anyone.testnet", T0 + MIN);
    let a = e.begin_claim(0);
    let late = T0 + 13 * MIN; // > pay_deadline (10) + grace (2)
    at(ME, late);
    let r = VerifierResult::Output(VerifierOutput { verdict: Verdict::NotPaid, reference: reference(&e, 0), key_fp: fp(), reason: "not_found".into() });
    e.on_verdict(0, a, r);
    let o = e.get_order(0).unwrap();
    assert_eq!(o.state, OrderState::AwaitingPayment);
    assert_eq!(o.claim_until.unwrap().0, late + 2 * MIN);
    lp_fault(&mut e, late + MIN);
    lp_fault(&mut e, late + MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Slashed);
}

#[test]
fn edge_timeout_from_awaiting_counts_then_second_timeout_slashes() {
    let mut e = demo();
    at("anyone.testnet", T0 + MIN);
    assert_eq!(e.begin_claim(0), 1);
    let t1 = T0 + MIN + VERIFY_TIMEOUT_NS + 1;
    at("anyone.testnet", t1);
    assert_eq!(e.begin_claim(0), 2);
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures, o.claimed_at.unwrap().0), (OrderState::Verifying, 1, t1));
    assert_eq!(o.last_failure_at.unwrap().0, T0 + MIN + VERIFY_TIMEOUT_NS, "stamped when due");
    // The first failure is stamped at T0 + MIN + VERIFY_TIMEOUT_NS, so the 6-min outage window ends
    // after t1. The second stuck check started at t1, before that: its timeout counts a failure but
    // cannot complete the window, so a fresh check starts.
    let t2 = t1 + VERIFY_TIMEOUT_NS + 1;
    at("anyone.testnet", t2);
    assert_eq!(e.begin_claim(0), 3);
    // Third stuck check, started at t2 (after the window): its timeout completes it and slashes.
    at("anyone.testnet", t2 + VERIFY_TIMEOUT_NS + 1);
    assert_eq!(e.begin_claim(0), 0);
    assert_eq!(state(&e), OrderState::Slashed);
}

#[test]
#[should_panic(expected = "a verification is already in flight")]
fn edge_timeout_reclaim_restarts_the_in_flight_clock() {
    let mut e = demo();
    at("anyone.testnet", T0 + MIN);
    e.begin_claim(0);
    let t1 = T0 + MIN + VERIFY_TIMEOUT_NS + 1;
    at("anyone.testnet", t1);
    e.begin_claim(0);
    at("anyone.testnet", t1 + 1);
    e.begin_claim(0);
}

#[test]
fn edge_slash_with_bond_exactly_the_order_amount() {
    let mut e = demo();
    // bond 20: second order of 10 makes locked == bond; slash order 1 and then order 0.
    at("kofi.testnet", T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    let r = unverifiable(&e, 1, "auth_error");
    fail(&mut e, 1, T0 + 2 * MIN + T_RETRY, r);
    let r = unverifiable(&e, 1, "auth_error");
    fail(&mut e, 1, T0 + 2 * MIN + 2 * T_RETRY, r);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.bond.0, lp.locked.0, lp.open_orders), (0, 0, 0));
}

#[test]
fn edge_lp_fault_is_sticky_through_a_later_outage() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    outage(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Slashed);
}

#[test]
fn edge_slash_adds_to_existing_claimable() {
    let mut e = demo();
    at(ME, T0);
    e.on_payout(acc(BUYER), U128(3 * USDC), Err(near_sdk::PromiseError::Failed));
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    assert_eq!(e.get_claimable(acc(BUYER)).0, 13 * USDC);
}

#[test]
#[should_panic(expected = "slash conditions not met")]
fn edge_public_slash_twice_with_other_order_open_fails() {
    let mut e = demo();
    at("kofi.testnet", T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    lp_fault(&mut e, T0 + MIN);
    lp_fault(&mut e, T0 + MIN + T_RETRY);
    e.slash(0);
}

#[test]
fn edge_outage_slash_event_says_not_lp_fault() {
    let mut e = demo();
    outage(&mut e, T0 + MIN);
    outage(&mut e, T0 + MIN + T_OUTAGE);
    assert_eq!(last_event().0, "order_slashed");
    assert_eq!(last_event().1["lp_fault"], json!(false));
}

#[test]
fn edge_not_paid_resets_lp_fault_so_outages_get_the_long_window() {
    let mut e = demo();
    lp_fault(&mut e, T0 + MIN);
    let r = VerifierResult::Output(VerifierOutput { verdict: Verdict::NotPaid, reference: reference(&e, 0), key_fp: fp(), reason: "not_found".into() });
    fail(&mut e, 0, T0 + 2 * MIN, r);
    outage(&mut e, T0 + 3 * MIN);
    outage(&mut e, T0 + 3 * MIN + T_RETRY);
    assert_eq!(state(&e), OrderState::Unverified);
}
