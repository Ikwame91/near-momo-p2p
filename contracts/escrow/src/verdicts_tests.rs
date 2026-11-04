//! M1.5 tests: claim, verdict handling (mock verifier), release, release delay, claimable payouts.

use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, OrderState, Params, Quote, SecretRef, Verdict, VerifierOutput, VerifierResult};
use near_sdk::json_types::U128;
use near_sdk::mock::MockAction;
use near_sdk::serde_json::{self, Value};
use near_sdk::test_utils::{get_created_receipts, get_logs, VMContextBuilder};
use near_sdk::{testing_env, NearToken, PromiseError};

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp.testnet";
const BUYER: &str = "ama.testnet";
const USDC: u128 = 1_000_000;
const MIN: u64 = 60_000_000_000;
const T0: u64 = 1_800_000_000_000_000_000;
const DEADLINE: u64 = T0 + 10 * MIN; // t_pay 10 min
const LAST_CLAIM: u64 = DEADLINE + 2 * MIN; // + t_grace 2 min

fn at(who: &str, time: u64) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        // Users attach NEAR for storage; the contract and the token never attach a deposit.
        .attached_deposit(NearToken::from_millinear(if who == ME || who == TOKEN { 0 } else { 100 }))
        .block_timestamp(time)
        .build());
}

fn fp() -> String {
    "ab".repeat(32)
}

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
    e.ft_on_transfer(acc(LP), U128(200 * USDC), "bond".into()).detach();
    e.ft_on_transfer(acc(LP), U128(300 * USDC), "liquidity".into()).detach();
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some(fp());
    e.lps.flush();
    e
}

/// Escrow with one open 10 USDC order (id 0) from a new buyer.
fn with_order() -> Escrow {
    let mut e = setup(params());
    at(BUYER, T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    e
}

fn reference(e: &Escrow) -> String {
    e.get_order(0).unwrap().reference
}

fn output(e: &Escrow, verdict: Verdict, reason: &str) -> VerifierResult {
    VerifierResult::Output(VerifierOutput { verdict, reference: reference(e), key_fp: fp(), reason: reason.into() })
}

/// Claim order 0 at `time`, then deliver `result` for that attempt as the contract itself.
fn verify(e: &mut Escrow, time: u64, result: VerifierResult) -> bool {
    at("anyone.testnet", time);
    let attempt = e.claim(0);
    at(ME, time);
    e.on_verdict(0, attempt, result)
}

fn state(e: &Escrow) -> OrderState {
    e.get_order(0).unwrap().state
}

fn events() -> Vec<String> {
    get_logs()
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .map(|j| serde_json::from_str::<Value>(j).unwrap()["event"].as_str().unwrap().to_string())
        .collect()
}

/// (receiver_id arg, amount arg) of every ft_transfer created so far.
fn ft_transfers() -> Vec<(String, String)> {
    get_created_receipts()
        .into_iter()
        .flat_map(|r| r.actions)
        .filter_map(|a| match a {
            MockAction::FunctionCallWeight { method_name, args, .. } if method_name == b"ft_transfer" => {
                let v: Value = serde_json::from_slice(&args).unwrap();
                Some((v["receiver_id"].as_str().unwrap().to_string(), v["amount"].as_str().unwrap().to_string()))
            }
            _ => None,
        })
        .collect()
}

// ---- claim ----

#[test]
fn claim_starts_verification_and_counts_attempts() {
    let mut e = with_order();
    at("anyone.testnet", T0 + MIN);
    assert_eq!(e.claim(0), 1);
    assert_eq!(state(&e), OrderState::Verifying);
    assert_eq!(events().last().unwrap(), "order_verifying");
}

#[test]
fn claim_at_exactly_deadline_plus_grace_is_allowed() {
    let mut e = with_order();
    at("anyone.testnet", LAST_CLAIM);
    e.claim(0);
    assert_eq!(state(&e), OrderState::Verifying);
}

#[test]
#[should_panic(expected = "claim window has passed")]
fn claim_after_deadline_plus_grace_fails() {
    let mut e = with_order();
    at("anyone.testnet", LAST_CLAIM + 1);
    e.claim(0);
}

#[test]
#[should_panic(expected = "a verification is already in flight")]
fn claim_while_verifying_fails() {
    // Only one verification in flight per order (SPEC §6).
    let mut e = with_order();
    at("anyone.testnet", T0);
    e.claim(0);
    e.claim(0);
}

#[test]
#[should_panic(expected = "no such order")]
fn claim_unknown_order_fails() {
    let mut e = with_order();
    e.claim(9);
}

// ---- PAID → release (inv. 1) ----

#[test]
fn paid_releases_usdc_to_buyer_and_closes_the_lock() {
    let mut e = with_order();
    assert!({ let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) });
    assert_eq!(state(&e), OrderState::Released);
    let lp = e.get_lp(acc(LP)).unwrap();
    // The USDC left the LP: liquidity stays at 290, locked back to 0.
    assert_eq!((lp.liquidity.0, lp.locked.0, lp.open_orders, lp.new_buyer_open.0), (290 * USDC, 0, 0, 0));
    // The verdict path schedules no transfer (PR #11 review): the USDC is owed to the buyer.
    assert!(ft_transfers().is_empty());
    assert_eq!(e.get_claimable(acc(BUYER)).0, 10 * USDC);
    assert_eq!(e.get_buyer_released(acc(LP), acc(BUYER)), 1);
    assert_eq!(events().last().unwrap(), "order_released");
}

#[test]
fn paid_with_wrong_key_fp_never_releases() {
    // inv. 10, T24: a verdict for a different key is UNVERIFIABLE and LP's fault.
    let mut e = with_order();
    let r = VerifierResult::Output(VerifierOutput {
        verdict: Verdict::Paid,
        reference: reference(&e),
        key_fp: "cd".repeat(32),
        reason: "ok".into(),
    });
    verify(&mut e, T0 + MIN, r);
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures, o.lp_fault), (OrderState::Unverified, 1, true));
    assert!(ft_transfers().is_empty(), "no USDC may move");
}

#[test]
fn paid_for_another_reference_never_releases() {
    let mut e = with_order();
    let r = VerifierResult::Output(VerifierOutput {
        verdict: Verdict::Paid,
        reference: "opr-00000000-7".into(),
        key_fp: fp(),
        reason: "ok".into(),
    });
    verify(&mut e, T0 + MIN, r);
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.lp_fault), (OrderState::Unverified, false));
    assert!(ft_transfers().is_empty());
}

#[test]
fn stale_verdict_for_an_older_attempt_is_ignored() {
    // T15: claim → failure → re-claim; a late callback for attempt 1 must not count.
    let mut e = with_order();
    { let r = output(&e, Verdict::Pending, "pending"); verify(&mut e, T0 + MIN, r) }; // attempt 1
    at("anyone.testnet", T0 + 2 * MIN);
    assert_eq!(e.claim(0), 2);
    at(ME, T0 + 2 * MIN);
    let r = output(&e, Verdict::Paid, "ok");
    assert!(!e.on_verdict(0, 1, r));
    assert_eq!(state(&e), OrderState::Verifying);
    assert!(ft_transfers().is_empty());
}

#[test]
fn verdict_when_not_verifying_is_ignored() {
    let mut e = with_order();
    at(ME, T0);
    let r = output(&e, Verdict::Paid, "ok");
    assert!(!e.on_verdict(0, 0, r));
    assert_eq!(state(&e), OrderState::AwaitingPayment);
}

#[test]
fn verdict_for_unknown_order_is_ignored() {
    let mut e = with_order();
    at(ME, T0);
    assert!(!e.on_verdict(42, 1, VerifierResult::RunFailed));
}

#[test]
#[should_panic(expected = "only the contract itself")]
fn verdict_from_a_stranger_is_rejected() {
    // T13 (also #[private] in wasm).
    let mut e = with_order();
    at("anyone.testnet", T0);
    let attempt = e.claim(0);
    at("attacker.testnet", T0);
    let r = output(&e, Verdict::Paid, "ok");
    e.on_verdict(0, attempt, r);
}

// ---- release delay (T6) ----

fn with_delay() -> Escrow {
    let mut p = params();
    p.release_delay_ns = 5 * MIN;
    let mut e = setup(p);
    at(BUYER, T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    e
}

#[test]
fn new_buyer_release_waits_for_the_delay() {
    let mut e = with_delay();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.release_at.map(|t| t.0)), (OrderState::ReleasePending, Some(T0 + 6 * MIN)));
    assert!(ft_transfers().is_empty());
    assert_eq!(e.get_lp(acc(LP)).unwrap().open_orders, 1, "still open: the bond stays locked");
}

#[test]
#[should_panic(expected = "release delay has not passed")]
fn finalize_before_release_at_fails() {
    let mut e = with_delay();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    at("anyone.testnet", T0 + 6 * MIN - 1);
    e.finalize_release(0);
}

#[test]
fn finalize_at_release_at_pays_the_buyer() {
    let mut e = with_delay();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    at("anyone.testnet", T0 + 6 * MIN);
    e.finalize_release(0);
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.release_at), (OrderState::Released, None));
    assert!(ft_transfers().is_empty());
    assert_eq!(e.get_claimable(acc(BUYER)).0, 10 * USDC);
    assert_eq!(e.get_lp(acc(LP)).unwrap().open_orders, 0);
}

#[test]
#[should_panic(expected = "order is not pending release")]
fn finalize_a_released_order_fails() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    e.finalize_release(0);
}

#[test]
fn experienced_buyer_is_released_without_delay() {
    let mut p = params();
    p.release_delay_ns = 5 * MIN;
    let mut e = setup(p);
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3);
    at(BUYER, T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), None);
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    assert_eq!(state(&e), OrderState::Released);
}

#[test]
fn three_releases_make_the_buyer_experienced_with_that_lp() {
    let mut e = setup(params());
    for id in 0..3u64 {
        at(BUYER, T0);
        let o = e.open_buy_order(acc(LP), U128(USDC), None);
        assert!(o.new_buyer);
        at("anyone.testnet", T0);
        let attempt = e.claim(id);
        at(ME, T0);
        let r = VerifierResult::Output(VerifierOutput {
            verdict: Verdict::Paid,
            reference: o.reference,
            key_fp: fp(),
            reason: "ok".into(),
        });
        e.on_verdict(id, attempt, r);
    }
    at(BUYER, T0);
    assert!(!e.open_buy_order(acc(LP), U128(USDC), None).new_buyer);
}

// ---- NOT_PAID ----

#[test]
fn not_paid_before_the_deadline_goes_back_to_awaiting_payment() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Pending, "pending"); verify(&mut e, T0 + MIN, r) };
    { let r = output(&e, Verdict::NotPaid, "not_found"); verify(&mut e, T0 + 2 * MIN, r) };
    let o = e.get_order(0).unwrap();
    assert_eq!(o.state, OrderState::AwaitingPayment);
    assert_eq!((o.failures, o.first_failure_at, o.lp_fault), (0, None, false), "streak reset");
    assert_eq!(e.get_lp(acc(LP)).unwrap().locked.0, 10 * USDC, "still locked for the buyer");
}

#[test]
fn not_paid_after_the_deadline_expires_the_order() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Pending, "pending"); verify(&mut e, T0 + MIN, r) };
    { let r = output(&e, Verdict::NotPaid, "not_found"); verify(&mut e, LAST_CLAIM + 1, r) }; // re-claim from UNVERIFIED
    assert_eq!(state(&e), OrderState::Expired);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.liquidity.0, lp.locked.0, lp.open_orders, lp.new_buyer_open.0), (300 * USDC, 0, 0, 0));
    assert!(ft_transfers().is_empty());
}

// ---- failures: PENDING / UNVERIFIABLE / run failed (inv. 2, D-012, D-017) ----

#[test]
fn pending_is_an_outage_failure_never_not_paid() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Pending, "pending"); verify(&mut e, T0 + MIN, r) };
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures, o.lp_fault), (OrderState::Unverified, 1, false));
    assert_eq!((o.first_failure_at.unwrap().0, o.last_failure_at.unwrap().0), (T0 + MIN, T0 + MIN));
    assert_eq!(e.get_lp(acc(LP)).unwrap().locked.0, 10 * USDC, "money stays locked");
}

#[test]
fn repeated_failures_keep_the_first_time_and_update_the_last() {
    let mut e = with_order();
    verify(&mut e, T0 + MIN, VerifierResult::RunFailed);
    { let r = output(&e, Verdict::Unverifiable, "server_error"); verify(&mut e, T0 + 30 * MIN, r) };
    let o = e.get_order(0).unwrap();
    assert_eq!((o.failures, o.first_failure_at.unwrap().0, o.last_failure_at.unwrap().0), (2, T0 + MIN, T0 + 30 * MIN));
    assert!(!o.lp_fault);
}

/// One escrow, one new order per reason (mocked storage persists within a test, so the escrow
/// cannot be set up twice): returns the lp_fault flag after an UNVERIFIABLE verdict with `reason`.
fn lp_fault_for(e: &mut Escrow, buyer: &str, reason: &str) -> bool {
    at(buyer, T0);
    let o = e.open_buy_order(acc(LP), U128(USDC), None);
    at("anyone.testnet", T0);
    let attempt = e.claim(o.id);
    at(ME, T0);
    let r = VerifierResult::Output(VerifierOutput {
        verdict: Verdict::Unverifiable,
        reference: o.reference,
        key_fp: fp(),
        reason: reason.into(),
    });
    e.on_verdict(o.id, attempt, r);
    e.get_order(o.id).unwrap().lp_fault
}

#[test]
fn auth_error_and_no_key_are_lp_fault() {
    let mut e = setup(params());
    assert!(lp_fault_for(&mut e, "b1.testnet", "auth_error"));
    assert!(lp_fault_for(&mut e, "b2.testnet", "no_key"));
}

#[test]
fn other_and_unknown_reasons_are_outages() {
    let mut e = setup(params());
    for (i, reason) in ["server_error", "network_error", "unclear_4xx", "malformed", "something_new", ""].iter().enumerate() {
        assert!(!lp_fault_for(&mut e, &format!("b{i}.testnet"), reason), "{reason}");
    }
}

#[test]
fn lp_fault_sticks_for_the_rest_of_the_streak() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Unverifiable, "auth_error"); verify(&mut e, T0 + MIN, r) };
    verify(&mut e, T0 + 2 * MIN, VerifierResult::RunFailed);
    assert!(e.get_order(0).unwrap().lp_fault);
}

#[test]
fn paid_after_failures_still_releases() {
    // An outage that clears: the buyer is paid as soon as the provider recovers.
    let mut e = with_order();
    verify(&mut e, T0 + MIN, VerifierResult::RunFailed);
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + 3 * 60 * MIN, r) };
    assert_eq!(state(&e), OrderState::Released);
}

// ---- payouts and claimable balances (inv. 6) ----

#[test]
fn failed_payout_becomes_claimable_again() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    at("anyone.testnet", T0 + MIN);
    let _ = e.withdraw_claimable(acc(BUYER)); // debits first
    assert_eq!(e.get_claimable(acc(BUYER)).0, 0);
    at(ME, T0 + MIN);
    assert!(!e.on_payout(acc(BUYER), U128(10 * USDC), Err(PromiseError::Failed)));
    assert_eq!(e.get_claimable(acc(BUYER)).0, 10 * USDC);
    assert_eq!(events().last().unwrap(), "claimable_credited");
}

#[test]
fn successful_payout_credits_nothing() {
    let mut e = with_order();
    at(ME, T0);
    assert!(e.on_payout(acc(BUYER), U128(10 * USDC), Ok(())));
    assert_eq!(e.get_claimable(acc(BUYER)).0, 0);
}

#[test]
fn anyone_can_retry_a_claimable_payout_to_its_owner() {
    let mut e = with_order();
    at(ME, T0);
    e.on_payout(acc(BUYER), U128(10 * USDC), Err(PromiseError::Failed));
    at("anyone.testnet", T0);
    let _ = e.withdraw_claimable(acc(BUYER));
    assert_eq!(e.get_claimable(acc(BUYER)).0, 0);
    assert_eq!(ft_transfers(), vec![(BUYER.to_string(), (10 * USDC).to_string())]);
}

#[test]
#[should_panic(expected = "nothing claimable")]
fn withdraw_claimable_with_nothing_fails() {
    let mut e = with_order();
    let _ = e.withdraw_claimable(acc(BUYER));
}

#[test]
#[should_panic(expected = "only the contract itself")]
fn payout_callback_from_a_stranger_is_rejected() {
    let mut e = with_order();
    at("attacker.testnet", T0);
    e.on_payout(acc("attacker.testnet"), U128(USDC), Err(PromiseError::Failed));
}

#[test]
fn not_paid_at_exactly_deadline_plus_grace_goes_back_to_awaiting_payment() {
    // Same boundary as claim (inclusive) and expire (strictly after): no gap, no overlap.
    let mut e = with_order();
    let r = output(&e, Verdict::NotPaid, "not_found");
    verify(&mut e, LAST_CLAIM, r);
    assert_eq!(state(&e), OrderState::AwaitingPayment);
    at("anyone.testnet", LAST_CLAIM + 1);
    e.expire_order(0);
    assert_eq!(state(&e), OrderState::Expired);
}

// ---- PR #11 review fix-ups ----

use crate::lp::{FT_TRANSFER_MIN_GAS, WITHDRAW_CALLBACK_GAS};
use crate::verdicts::VERIFY_TIMEOUT_NS;
use crate::MIN_CALLBACK_GAS_TGAS;
use near_sdk::serde_json::json;
use near_sdk::Gas;

/// (event, data) of every NEP-297 event in the current logs.
fn event_data() -> Vec<(String, Value)> {
    get_logs()
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .map(|j| {
            let v: Value = serde_json::from_str(j).unwrap();
            (v["event"].as_str().unwrap().to_string(), v["data"].clone())
        })
        .collect()
}

#[test]
fn paid_verdict_fits_in_the_minimum_callback_gas() {
    // H1: the verdict path schedules no promise, so even the smallest allowed callback budget
    // (MIN_CALLBACK_GAS_TGAS) is enough; a panic here would leave the order stuck in VERIFYING.
    let mut e = with_order();
    at("anyone.testnet", T0 + MIN);
    let attempt = e.claim(0);
    let r = output(&e, Verdict::Paid, "ok");
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(ME))
        .block_timestamp(T0 + MIN)
        .prepaid_gas(Gas::from_tgas(MIN_CALLBACK_GAS_TGAS))
        .build());
    assert!(e.on_verdict(0, attempt, r));
    assert_eq!(state(&e), OrderState::Released);
    assert!(get_created_receipts().is_empty(), "no promise from the verdict path");
}

#[test]
fn withdraw_claimable_creates_exact_transfer_and_callback() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    at("anyone.testnet", T0 + MIN);
    let _ = e.withdraw_claimable(acc(BUYER));
    let mut calls = vec![];
    for r in get_created_receipts() {
        for a in r.actions {
            if let MockAction::FunctionCallWeight { method_name, args, attached_deposit, prepaid_gas, gas_weight, .. } = a {
                let v: Value = serde_json::from_slice(&args).unwrap();
                calls.push((r.receiver_id.to_string(), String::from_utf8(method_name).unwrap(), v, attached_deposit, prepaid_gas, gas_weight.0));
            }
        }
    }
    assert_eq!(calls.len(), 2);
    let (to, m, args, dep, gas, w) = &calls[0];
    assert_eq!((to.as_str(), m.as_str()), (TOKEN, "ft_transfer"));
    assert_eq!((args["receiver_id"].as_str(), args["amount"].as_str()), (Some(BUYER), Some("10000000")));
    assert_eq!((*dep, *gas, *w), (NearToken::from_yoctonear(1), FT_TRANSFER_MIN_GAS, 1));
    let (to, m, args, dep, gas, w) = &calls[1];
    assert_eq!((to.as_str(), m.as_str()), (ME, "on_payout"));
    assert_eq!(args, &json!({"account": BUYER, "amount": "10000000"}));
    assert_eq!((*dep, *gas, *w), (NearToken::from_yoctonear(0), WITHDRAW_CALLBACK_GAS, 0));
}

#[test]
fn two_failed_payouts_add_up() {
    let mut e = with_order();
    at(ME, T0);
    e.on_payout(acc(BUYER), U128(10 * USDC), Err(PromiseError::Failed));
    e.on_payout(acc(BUYER), U128(10 * USDC), Err(PromiseError::Failed));
    assert_eq!(e.get_claimable(acc(BUYER)).0, 20 * USDC);
    assert_eq!(event_data().last().unwrap(), &("claimable_credited".into(), json!({"account": BUYER, "amount": "10000000"})));
}

#[test]
#[should_panic(expected = "order cannot be claimed in its current state")]
fn claim_on_release_pending_fails() {
    let mut e = with_delay();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    e.claim(0);
}

#[test]
#[should_panic(expected = "order cannot be claimed in its current state")]
fn claim_on_released_fails() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    e.claim(0);
}

#[test]
#[should_panic(expected = "order cannot be claimed in its current state")]
fn claim_on_expired_fails() {
    let mut e = with_order();
    at("anyone.testnet", LAST_CLAIM + 1);
    e.expire_order(0);
    e.claim(0);
}

#[test]
fn verdict_events_carry_the_right_data() {
    let mut e = with_order();
    at("anyone.testnet", T0 + MIN);
    e.claim(0);
    assert_eq!(event_data().last().unwrap(), &("order_verifying".into(), json!({"order_id": 0, "attempt": 1})));
    at(ME, T0 + MIN);
    let r = output(&e, Verdict::Unverifiable, "auth_error");
    e.on_verdict(0, 1, r);
    assert_eq!(
        event_data().last().unwrap(),
        &("order_unverified".into(), json!({"order_id": 0, "attempt": 1, "failures": 1, "lp_fault": true}))
    );
    at("anyone.testnet", T0 + 2 * MIN);
    e.claim(0);
    at(ME, T0 + 2 * MIN);
    let r = output(&e, Verdict::Paid, "ok");
    e.on_verdict(0, 2, r);
    assert_eq!(
        event_data().last().unwrap(),
        &("order_released".into(), json!({"order_id": 0, "buyer": BUYER, "amount": "10000000"}))
    );
}

#[test]
fn not_paid_and_release_pending_events_carry_the_right_data() {
    let mut e = with_delay();
    { let r = output(&e, Verdict::NotPaid, "not_found"); verify(&mut e, T0 + MIN, r) };
    assert_eq!(event_data().last().unwrap(), &("order_not_paid".into(), json!({"order_id": 0, "expired": false})));
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + 2 * MIN, r) };
    assert_eq!(
        event_data().last().unwrap(),
        &("order_release_pending".into(), json!({"order_id": 0, "release_at": (T0 + 7 * MIN).to_string()}))
    );
}

#[test]
fn not_paid_resets_the_whole_streak() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Unverifiable, "auth_error"); verify(&mut e, T0 + MIN, r) };
    { let r = output(&e, Verdict::NotPaid, "not_found"); verify(&mut e, T0 + 2 * MIN, r) };
    let o = e.get_order(0).unwrap();
    assert_eq!((o.failures, o.first_failure_at, o.last_failure_at, o.lp_fault), (0, None, None, false));
}

#[test]
fn pending_after_the_deadline_stays_unverified_never_expired() {
    // T35: a charge still pending after deadline + grace must not expire the order.
    let mut e = with_order();
    { let r = output(&e, Verdict::Pending, "pending"); verify(&mut e, T0 + MIN, r) };
    { let r = output(&e, Verdict::Pending, "pending"); verify(&mut e, LAST_CLAIM + 1, r) };
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures), (OrderState::Unverified, 2));
    assert_eq!(e.get_lp(acc(LP)).unwrap().locked.0, 10 * USDC);
    assert!(ft_transfers().is_empty());
    assert_eq!(e.get_claimable(acc(BUYER)).0, 0);
}

#[test]
fn repeated_failures_leave_the_order_unverified_and_move_no_money() {
    // T16.
    let mut e = with_order();
    verify(&mut e, T0 + MIN, VerifierResult::RunFailed);
    { let r = output(&e, Verdict::Unverifiable, "server_error"); verify(&mut e, T0 + 2 * MIN, r) };
    assert_eq!(state(&e), OrderState::Unverified);
    assert!(ft_transfers().is_empty());
    assert_eq!(e.get_claimable(acc(BUYER)).0, 0);
}

#[test]
fn slow_not_paid_for_an_early_check_gives_a_fresh_grace_period() {
    // M1 (PR #11 review): the check started before the deadline but its answer came after
    // deadline + grace; the buyer may have paid on time, so the order must not expire yet.
    let mut e = with_order();
    at("anyone.testnet", DEADLINE - MIN);
    let attempt = e.claim(0);
    at(ME, LAST_CLAIM + 5 * MIN);
    let r = output(&e, Verdict::NotPaid, "not_found");
    e.on_verdict(0, attempt, r);
    let o = e.get_order(0).unwrap();
    assert_eq!(o.state, OrderState::AwaitingPayment);
    assert_eq!(o.claim_until.map(|t| t.0), Some(LAST_CLAIM + 7 * MIN)); // now + t_grace
    at("anyone.testnet", LAST_CLAIM + 7 * MIN); // the buyer can claim again until then
    e.claim(0);
    assert_eq!(state(&e), OrderState::Verifying);
}

#[test]
#[should_panic(expected = "order has not expired yet")]
fn order_cannot_be_expired_during_the_fresh_grace_period() {
    let mut e = with_order();
    at("anyone.testnet", DEADLINE - MIN);
    let attempt = e.claim(0);
    at(ME, LAST_CLAIM + 5 * MIN);
    let r = output(&e, Verdict::NotPaid, "not_found");
    e.on_verdict(0, attempt, r);
    at("anyone.testnet", LAST_CLAIM + 7 * MIN);
    e.expire_order(0);
}

#[test]
fn not_paid_for_a_check_started_after_the_deadline_expires_after_grace() {
    // A check that started after the deadline can only find a late payment: no extension.
    let mut e = with_order();
    at("anyone.testnet", DEADLINE + MIN); // inside the grace window
    let attempt = e.claim(0);
    at(ME, LAST_CLAIM + 1);
    let r = output(&e, Verdict::NotPaid, "not_found");
    e.on_verdict(0, attempt, r);
    assert_eq!(state(&e), OrderState::Expired);
}

#[test]
#[should_panic(expected = "a verification is already in flight")]
fn stuck_verification_cannot_be_reclaimed_before_the_timeout() {
    let mut e = with_order();
    at("anyone.testnet", T0);
    e.claim(0);
    at("anyone.testnet", T0 + VERIFY_TIMEOUT_NS);
    e.claim(0);
}

#[test]
fn stuck_verification_can_be_reclaimed_after_the_timeout() {
    // H2: a callback that never arrives (or panicked) must not freeze the order and the bond.
    let mut e = with_order();
    at("anyone.testnet", T0);
    assert_eq!(e.claim(0), 1);
    at("anyone.testnet", T0 + VERIFY_TIMEOUT_NS + 1);
    assert_eq!(e.claim(0), 2);
    let o = e.get_order(0).unwrap();
    assert_eq!((o.state, o.failures, o.lp_fault), (OrderState::Verifying, 1, false), "counted as an outage");
    at(ME, T0 + VERIFY_TIMEOUT_NS + 2);
    let r = output(&e, Verdict::Paid, "ok");
    assert!(!e.on_verdict(0, 1, r), "the late answer for attempt 1 is ignored");
}

#[test]
fn release_adds_to_an_existing_claimable_balance() {
    // inv. 6: a buyer with USDC already waiting (an earlier failed payout) must not lose it.
    let mut e = with_order();
    at(ME, T0);
    e.on_payout(acc(BUYER), U128(3 * USDC), Err(PromiseError::Failed));
    { let r = output(&e, Verdict::Paid, "ok"); verify(&mut e, T0 + MIN, r) };
    assert_eq!(e.get_claimable(acc(BUYER)).0, 13 * USDC);
}

#[test]
fn not_paid_expiry_event_says_expired() {
    let mut e = with_order();
    { let r = output(&e, Verdict::Pending, "pending"); verify(&mut e, T0 + MIN, r) };
    { let r = output(&e, Verdict::NotPaid, "not_found"); verify(&mut e, LAST_CLAIM + 1, r) };
    assert_eq!(event_data().last().unwrap(), &("order_not_paid".into(), json!({"order_id": 0, "expired": true})));
}
