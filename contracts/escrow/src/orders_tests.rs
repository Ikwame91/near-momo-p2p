//! M1.4 tests: buy order open / expire, references, GHS rounding, limits, storage, views.

use crate::orders::{ghs_amount_pesewas, MAX_PAGE};
use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, OrderState, Quote, SecretRef};
use near_sdk::json_types::U128;
use near_sdk::mock::MockAction;
use near_sdk::serde_json::{self, json, Value};
use near_sdk::test_utils::{get_created_receipts, get_logs, VMContextBuilder};
use near_sdk::{testing_env, NearToken};

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp.testnet";
const BUYER: &str = "ama.testnet";
const USDC: u128 = 1_000_000;
const MIN: u64 = 60_000_000_000;
const T0: u64 = 1_800_000_000_000_000_000; // a block time in ns

fn at(who: &str, deposit: NearToken, time: u64) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(deposit)
        .block_timestamp(time)
        .build());
}

fn near(milli: u128) -> NearToken {
    NearToken::from_millinear(milli)
}

fn quote(max: u128) -> Quote {
    Quote {
        rate_pesewas_per_usdc: U128(1_500), // GHS 15.00 per USDC
        min_usdc: U128(USDC),
        max_usdc: U128(max),
        new_buyer_exposure_max: U128(100 * USDC),
    }
}

/// Buy-side LP with `bond` and `liquidity`, fingerprinted, quote max 200 USDC.
fn with_lp(provider: &str, bond: u128, liquidity: u128) -> Escrow {
    at(ME, near(0), T0);
    let mut e = init_with(verifier(), params());
    at(LP, near(100), T0);
    e.register_lp(provider.into(), SecretRef { profile: "sec".into(), account_id: acc(LP) }, quote(200 * USDC));
    at(TOKEN, near(0), T0);
    e.ft_on_transfer(acc(LP), U128(bond), "bond".into()).detach();
    e.ft_on_transfer(acc(LP), U128(liquidity), "liquidity".into()).detach();
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some("ab".repeat(32));
    e.lps.flush(); // write test-only edits now, so they are not counted in a later order's storage
    e
}

fn ready() -> Escrow {
    with_lp("paystack", 200 * USDC, 300 * USDC)
}

fn open(e: &mut Escrow, buyer: &str, usdc: u128) -> crate::Order {
    at(buyer, near(100), T0);
    e.open_buy_order(acc(LP), U128(usdc), None)
}

fn last_event() -> (String, Value) {
    let l = get_logs().into_iter().rev().find(|l| l.starts_with("EVENT_JSON:")).expect("an event");
    let v: Value = serde_json::from_str(l.strip_prefix("EVENT_JSON:").unwrap()).unwrap();
    (v["event"].as_str().unwrap().to_string(), v["data"].clone())
}

// ---- GHS amount (SPEC §4.1) ----

#[test]
fn ghs_amount_exact_and_rounded_up() {
    assert_eq!(ghs_amount_pesewas(10 * USDC, 1_500), 15_000); // 10 USDC × GHS 15 = GHS 150.00
    assert_eq!(ghs_amount_pesewas(1_000, 1_500), 2); // 1.5 pesewas → 2 (round up, protects the LP)
    assert_eq!(ghs_amount_pesewas(1, 1_500), 1); // 0.0015 → 1
    assert_eq!(ghs_amount_pesewas(3 * USDC + 1, 1_000), 3_001); // 3000.001 → 3001
}

#[test]
#[should_panic(expected = "amount × rate overflows")]
fn ghs_amount_overflow_fails() {
    ghs_amount_pesewas(u128::MAX, 2);
}

#[test]
#[should_panic(expected = "GHS amount too large")]
fn ghs_amount_above_u64_fails() {
    ghs_amount_pesewas(u64::MAX as u128 * 1_000_000 + 1_000_000, 1);
}

// ---- open ----

#[test]
fn open_creates_order_locks_liquidity_and_emits() {
    let mut e = ready();
    let o = open(&mut e, BUYER, 10 * USDC);
    let tag = e.get_config().deploy_tag;
    assert_eq!(o.id, 0);
    assert_eq!(o.reference, format!("opr-{tag}-0"));
    assert_eq!(o.state, OrderState::AwaitingPayment);
    assert_eq!(o.ghs_amount_pesewas.0, 15_000);
    assert_eq!((o.opened_at.0, o.pay_deadline.0), (T0, T0 + 10 * MIN));
    assert!(o.new_buyer);
    assert_eq!(e.get_order(0), Some(o.clone()));
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.liquidity.0, lp.locked.0, lp.open_orders, lp.new_buyer_open.0), (290 * USDC, 10 * USDC, 1, 10 * USDC));
    assert_eq!(
        last_event(),
        (
            "order_opened".into(),
            json!({"order_id": 0, "lp": LP, "buyer": BUYER, "usdc_amount": "10000000",
                   "ghs_amount_pesewas": "15000", "reference": format!("opr-{tag}-0"),
                   "pay_deadline": (T0 + 10 * MIN).to_string()})
        )
    );
}

#[test]
fn order_ids_and_references_increment() {
    let mut e = ready();
    let a = open(&mut e, BUYER, USDC);
    let b = open(&mut e, "kofi.testnet", USDC);
    assert_eq!((a.id, b.id), (0, 1));
    assert_ne!(a.reference, b.reference);
    assert!(b.reference.ends_with("-1"));
}

#[test]
#[should_panic(expected = "for storage")]
fn open_without_storage_deposit_fails() {
    let mut e = ready();
    at(BUYER, near(0), T0);
    e.open_buy_order(acc(LP), U128(USDC), None);
}

#[test]
fn open_refunds_storage_excess_to_buyer() {
    let mut e = ready();
    at(BUYER, NearToken::from_near(1), T0);
    e.open_buy_order(acc(LP), U128(USDC), None);
    let refunds: Vec<NearToken> = get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id.as_str() == BUYER)
        .flat_map(|r| r.actions)
        .filter_map(|a| if let MockAction::Transfer { deposit, .. } = a { Some(deposit) } else { None })
        .collect();
    assert_eq!(refunds.len(), 1);
    assert!(refunds[0] > near(990), "most of 1 NEAR comes back: {}", refunds[0]);
}

#[test]
#[should_panic(expected = "an LP cannot buy from itself")]
fn lp_cannot_buy_from_itself() {
    let mut e = ready();
    open(&mut e, LP, USDC);
}

#[test]
#[should_panic(expected = "not a registered LP")]
fn open_with_unknown_lp_fails() {
    let mut e = ready();
    at(BUYER, near(100), T0);
    e.open_buy_order(acc("nobody.testnet"), U128(USDC), None);
}

#[test]
#[should_panic(expected = "LP does not serve buy orders")]
fn open_with_sell_side_lp_fails() {
    let mut e = with_lp("pawapay", 200 * USDC, 300 * USDC);
    open(&mut e, BUYER, USDC);
}

#[test]
#[should_panic(expected = "LP is paused")]
fn open_with_paused_lp_fails() {
    // T28 at the public entry point.
    let mut e = ready();
    at(LP, NearToken::from_yoctonear(1), T0);
    e.pause_lp();
    open(&mut e, BUYER, USDC);
}

#[test]
#[should_panic(expected = "LP key not fingerprinted yet")]
fn open_before_key_fp_fails() {
    let mut e = ready();
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = None;
    open(&mut e, BUYER, USDC);
}

#[test]
#[should_panic(expected = "amount below the LP's minimum")]
fn open_below_quote_min_fails() {
    let mut e = ready();
    open(&mut e, BUYER, USDC - 1);
}

#[test]
#[should_panic(expected = "amount above the LP's maximum")]
fn open_above_quote_max_fails() {
    let mut e = ready();
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3); // not new, so only the quote limit applies
    open(&mut e, BUYER, 200 * USDC + 1);
}

// ---- new-buyer limits (inv. 14, T29) ----

#[test]
#[should_panic(expected = "amount above the new-buyer cap")]
fn new_buyer_order_above_cap_fails() {
    let mut e = ready();
    open(&mut e, BUYER, 50 * USDC + 1); // demo new_buyer_cap = 50 USDC
}

#[test]
fn new_buyer_order_at_cap_is_allowed() {
    let mut e = ready();
    assert!(open(&mut e, BUYER, 50 * USDC).new_buyer);
}

#[test]
fn experienced_buyer_is_not_capped() {
    let mut e = ready();
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3); // new_buyer_orders = 3
    let o = open(&mut e, BUYER, 120 * USDC);
    assert!(!o.new_buyer);
    assert_eq!(e.get_lp(acc(LP)).unwrap().new_buyer_open.0, 0);
}

#[test]
fn buyer_with_fewer_released_orders_is_still_new() {
    let mut e = ready();
    e.buyer_released.insert((acc(LP), acc(BUYER)), 2);
    assert!(open(&mut e, BUYER, USDC).new_buyer);
    assert_eq!(e.get_buyer_released(acc(LP), acc(BUYER)), 2);
}

#[test]
#[should_panic(expected = "LP's new-buyer exposure limit reached")]
fn many_new_buyers_hit_the_lp_exposure_limit() {
    // Many fresh accounts (T29): exposure 100 USDC, each order 50 → the third fails.
    let mut e = ready();
    open(&mut e, "n1.testnet", 50 * USDC);
    open(&mut e, "n2.testnet", 50 * USDC);
    open(&mut e, "n3.testnet", USDC);
}

// ---- bond rule at order level (inv. 5) ----

#[test]
#[should_panic(expected = "bond does not cover this order")]
fn orders_beyond_the_bond_fail() {
    let mut e = with_lp("paystack", 10 * USDC, 300 * USDC);
    open(&mut e, BUYER, 6 * USDC);
    open(&mut e, "kofi.testnet", 5 * USDC);
}

#[test]
#[should_panic(expected = "bond is locked while orders are open")]
fn withdraw_bond_with_an_open_buy_order_fails() {
    let mut e = ready();
    open(&mut e, BUYER, USDC);
    at(LP, NearToken::from_yoctonear(1), T0);
    let _ = e.withdraw_bond(U128(USDC));
}

// ---- expire ----

#[test]
#[should_panic(expected = "order has not expired yet")]
fn expire_before_deadline_plus_grace_fails() {
    let mut e = ready();
    open(&mut e, BUYER, USDC);
    at("anyone.testnet", near(0), T0 + 10 * MIN + 2 * MIN); // exactly deadline + grace
    e.expire_order(0);
}

#[test]
fn expire_after_grace_unlocks_everything() {
    let mut e = ready();
    open(&mut e, BUYER, 10 * USDC);
    at("anyone.testnet", near(0), T0 + 12 * MIN + 1);
    e.expire_order(0);
    assert_eq!(e.get_order(0).unwrap().state, OrderState::Expired);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.liquidity.0, lp.locked.0, lp.open_orders, lp.new_buyer_open.0), (300 * USDC, 0, 0, 0));
    assert_eq!(last_event(), ("order_expired".into(), json!({"order_id": 0})));
}

#[test]
#[should_panic(expected = "order is not awaiting payment")]
fn expire_twice_fails() {
    let mut e = ready();
    open(&mut e, BUYER, USDC);
    at("anyone.testnet", near(0), T0 + 13 * MIN);
    e.expire_order(0);
    e.expire_order(0);
}

#[test]
#[should_panic(expected = "no such order")]
fn expire_unknown_order_fails() {
    let mut e = ready();
    e.expire_order(7);
}

#[test]
fn after_expiry_the_lp_can_withdraw_its_bond() {
    let mut e = ready();
    open(&mut e, BUYER, USDC);
    at("anyone.testnet", near(0), T0 + 13 * MIN);
    e.expire_order(0);
    at(LP, NearToken::from_yoctonear(1), T0 + 13 * MIN);
    let _ = e.withdraw_bond(U128(200 * USDC));
    assert_eq!(e.get_lp(acc(LP)).unwrap().bond.0, 0);
}

#[test]
fn paused_lp_orders_can_still_expire() {
    let mut e = ready();
    open(&mut e, BUYER, USDC);
    at(LP, NearToken::from_yoctonear(1), T0);
    e.pause_lp();
    at("anyone.testnet", near(0), T0 + 13 * MIN);
    e.expire_order(0);
    assert_eq!(e.get_lp(acc(LP)).unwrap().open_orders, 0);
}

// ---- views ----

#[test]
fn list_views_page_by_lp_and_buyer() {
    let mut e = ready();
    for _ in 0..3 {
        open(&mut e, BUYER, USDC);
    }
    open(&mut e, "kofi.testnet", USDC);
    let ids = |v: Vec<crate::Order>| v.into_iter().map(|o| o.id).collect::<Vec<_>>();
    assert_eq!(ids(e.list_orders_by_lp(acc(LP), 0, 10)), vec![0, 1, 2, 3]);
    assert_eq!(ids(e.list_orders_by_lp(acc(LP), 1, 2)), vec![1, 2]);
    assert_eq!(ids(e.list_orders_by_buyer(acc(BUYER), 0, 10)), vec![0, 1, 2]);
    assert_eq!(ids(e.list_orders_by_buyer(acc("kofi.testnet"), 0, 10)), vec![3]);
    assert!(e.list_orders_by_buyer(acc("nobody.testnet"), 0, 10).is_empty());
    assert!(e.list_orders_by_lp(acc(LP), 10, 10).is_empty());
}

#[test]
fn list_view_limit_is_capped() {
    let mut e = with_lp("paystack", 200 * USDC, 300 * USDC);
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3);
    for _ in 0..(MAX_PAGE + 5) {
        open(&mut e, BUYER, USDC);
    }
    assert_eq!(e.list_orders_by_buyer(acc(BUYER), 0, 1_000).len(), MAX_PAGE as usize);
}

#[test]
fn order_json_uses_strings_for_large_numbers() {
    let mut e = ready();
    let o = open(&mut e, BUYER, USDC);
    let v = serde_json::to_value(&o).unwrap();
    assert_eq!(v["state"], "AWAITING_PAYMENT");
    assert!(v["pay_deadline"].is_string(), "ns timestamps exceed JavaScript's 2^53");
    assert!(v["usdc_amount"].is_string());
}

// ---- PR #10 review fix-ups ----

use crate::orders::{ORDER_GROWTH_BYTES, RECORD_OVERHEAD};
use near_sdk::env;
use near_sdk::json_types::U64;

/// Register another fingerprinted paystack LP `lp` (same quote/bond/liquidity as `ready()`).
fn add_lp(e: &mut Escrow, lp: &str) {
    at(lp, near(100), T0);
    e.register_lp("paystack".into(), SecretRef { profile: "sec".into(), account_id: acc(lp) }, quote(200 * USDC));
    at(TOKEN, near(0), T0);
    e.ft_on_transfer(acc(lp), U128(200 * USDC), "bond".into()).detach();
    e.ft_on_transfer(acc(lp), U128(300 * USDC), "liquidity".into()).detach();
    e.lps.get_mut(&acc(lp)).unwrap().key_fp = Some("ab".repeat(32));
    e.lps.flush();
}

fn refund_to(who: &str) -> Vec<NearToken> {
    get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id.as_str() == who)
        .flat_map(|r| r.actions)
        .filter_map(|a| if let MockAction::Transfer { deposit, .. } = a { Some(deposit) } else { None })
        .collect()
}

/// Bytes charged beyond what open writes for a first order of (lp, buyer).
fn later_bytes(lp: &str, buyer: &str) -> u64 {
    let claimable = RECORD_OVERHEAD + 1 + (4 + buyer.len() as u64) + 16;
    ORDER_GROWTH_BYTES + claimable + RECORD_OVERHEAD + 1 + (4 + lp.len() as u64) + (4 + buyer.len() as u64) + 4
}

#[test]
fn storage_charge_is_exactly_written_bytes_plus_reserved_growth() {
    let mut e = ready();
    let paid = NearToken::from_near(1);
    at(BUYER, paid, T0); // measure inside the same context as the call
    let before = env::storage_usage();
    e.open_buy_order(acc(LP), U128(USDC), None);
    // Force every map to write NOW, independently of what the contract flushed itself, so a
    // missing flush (= bytes stored but not charged) makes this test fail.
    e.lps.flush();
    e.orders.flush();
    e.lp_orders.flush();
    e.lp_order_count.flush();
    e.buyer_orders.flush();
    e.buyer_order_count.flush();
    e.buyer_released.flush();
    let written = env::storage_usage() - before;
    let charged = env::storage_byte_cost().saturating_mul((written + later_bytes(LP, BUYER)) as u128);
    assert_eq!(refund_to(BUYER), vec![paid.saturating_sub(charged)]);
}

/// Exact cost of a first order of (lp, buyer), read from the refund of an overpaid open.
fn first_order_cost(e: &mut Escrow, lp: &str, buyer: &str) -> NearToken {
    let paid = NearToken::from_near(1);
    at(buyer, paid, T0);
    e.open_buy_order(acc(lp), U128(USDC), None);
    paid.saturating_sub(refund_to(buyer)[0])
}

#[test]
fn exact_storage_deposit_succeeds_without_refund() {
    // Same-length account ids, so the second first-order costs the same as the first.
    let mut e = ready();
    add_lp(&mut e, "lq.testnet");
    let cost = first_order_cost(&mut e, LP, "b1.testnet");
    at("b2.testnet", cost, T0);
    e.open_buy_order(acc("lq.testnet"), U128(USDC), None);
    assert!(refund_to("b2.testnet").is_empty());
}

#[test]
#[should_panic(expected = "for storage")]
fn storage_deposit_one_yocto_short_fails() {
    let mut e = ready();
    add_lp(&mut e, "lq.testnet");
    let cost = first_order_cost(&mut e, LP, "b1.testnet");
    at("b2.testnet", cost.saturating_sub(NearToken::from_yoctonear(1)), T0);
    e.open_buy_order(acc("lq.testnet"), U128(USDC), None);
}

#[test]
fn cost_per_order_does_not_grow_with_history() {
    // O(1) index (PR #10 review): the 2nd and the 40th order of the same (lp, buyer) write the
    // same number of bytes.
    let mut e = ready();
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3);
    e.buyer_released.flush();
    let mut written = vec![];
    for i in 0..40u64 {
        at(BUYER, near(100), T0);
        let before = env::storage_usage();
        e.open_buy_order(acc(LP), U128(USDC), None);
        // The reference "opr-<tag>-<id>" grows by one byte per extra digit of the id; the rest
        // (order record + index entries) must be constant.
        let digits = i.to_string().len() as u64;
        written.push(env::storage_usage() - before - digits);
    }
    assert!(written[1..].iter().all(|w| *w == written[1]), "{written:?}");
    assert_eq!(e.count_orders_by_lp(acc(LP)), 40);
    assert_eq!(e.count_orders_by_buyer(acc(BUYER)), 40);
}

#[test]
fn new_buyer_exposure_at_exactly_the_limit_is_allowed() {
    let mut e = ready();
    open(&mut e, "n1.testnet", 50 * USDC);
    open(&mut e, "n2.testnet", 50 * USDC); // 100 = new_buyer_exposure_max
    assert_eq!(e.get_lp(acc(LP)).unwrap().new_buyer_open.0, 100 * USDC);
}

#[test]
fn order_at_exactly_the_quote_max_is_allowed() {
    let mut e = ready();
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3);
    assert_eq!(open(&mut e, BUYER, 200 * USDC).usdc_amount.0, 200 * USDC);
}

#[test]
fn rate_change_after_open_does_not_change_the_order() {
    // T10: the GHS amount is fixed when the order opens.
    let mut e = ready();
    open(&mut e, BUYER, 10 * USDC);
    let mut q = quote(200 * USDC);
    q.rate_pesewas_per_usdc = U128(9_999);
    at(LP, NearToken::from_yoctonear(1), T0);
    e.update_quote(q);
    assert_eq!(e.get_order(0).unwrap().ghs_amount_pesewas.0, 15_000);
}

#[test]
#[should_panic(expected = "GHS amount above the buyer's maximum")]
fn rate_raised_before_open_is_caught_by_the_buyers_maximum() {
    let mut e = ready();
    let mut q = quote(200 * USDC);
    q.rate_pesewas_per_usdc = U128(1_600); // LP raises the rate just before the buyer's call lands
    at(LP, NearToken::from_yoctonear(1), T0);
    e.update_quote(q);
    at(BUYER, near(100), T0);
    e.open_buy_order(acc(LP), U128(10 * USDC), Some(U64(15_000)));
}

#[test]
fn buyers_maximum_equal_to_the_amount_is_allowed() {
    let mut e = ready();
    at(BUYER, near(100), T0);
    let o = e.open_buy_order(acc(LP), U128(10 * USDC), Some(U64(15_000)));
    assert_eq!(o.ghs_amount_pesewas.0, 15_000);
}

#[test]
fn history_with_one_lp_does_not_make_a_buyer_experienced_with_another() {
    // D-020: a colluding LP cannot "graduate" a buyer for honest LPs (T29).
    let mut e = ready();
    add_lp(&mut e, "lq.testnet");
    e.buyer_released.insert((acc("lq.testnet"), acc(BUYER)), 3);
    assert!(open(&mut e, BUYER, USDC).new_buyer, "still new with LP");
    assert_eq!(e.get_buyer_released(acc(LP), acc(BUYER)), 0);
    assert_eq!(e.get_buyer_released(acc("lq.testnet"), acc(BUYER)), 3);
}

#[test]
fn mixed_new_and_experienced_orders_expire_cleanly() {
    let mut e = ready();
    open(&mut e, "n1.testnet", 10 * USDC);
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3);
    open(&mut e, BUYER, 20 * USDC);
    assert_eq!(e.get_lp(acc(LP)).unwrap().new_buyer_open.0, 10 * USDC);
    at("anyone.testnet", near(0), T0 + 13 * MIN);
    e.expire_order(1);
    assert_eq!(e.get_lp(acc(LP)).unwrap().new_buyer_open.0, 10 * USDC);
    e.expire_order(0);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.new_buyer_open.0, lp.locked.0, lp.liquidity.0, lp.open_orders), (0, 0, 300 * USDC, 0));
}

#[test]
#[should_panic(expected = "not enough LP liquidity")]
fn open_beyond_liquidity_fails() {
    let mut e = with_lp("paystack", 200 * USDC, 5 * USDC);
    open(&mut e, BUYER, 10 * USDC);
}

#[test]
#[should_panic(expected = "unlock exceeds new-buyer exposure")]
fn unlock_new_buyer_flag_without_new_buyer_lock_fails() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), USDC, false);
    e.unlock(&acc(LP), USDC, true, true);
}

#[test]
fn ghs_amount_at_u64_max_is_allowed() {
    assert_eq!(ghs_amount_pesewas(u64::MAX as u128 * 1_000_000, 1), u64::MAX);
}

#[test]
fn list_views_page_from_the_newest_end_with_counts() {
    let mut e = ready();
    e.buyer_released.insert((acc(LP), acc(BUYER)), 3);
    for _ in 0..5 {
        open(&mut e, BUYER, USDC);
    }
    let n = e.count_orders_by_buyer(acc(BUYER));
    let newest: Vec<u64> = e.list_orders_by_buyer(acc(BUYER), n - 2, 2).into_iter().map(|o| o.id).collect();
    assert_eq!(newest, vec![3, 4]);
    assert_eq!(e.count_orders_by_lp(acc("nobody.testnet")), 0);
}

#[test]
fn new_order_has_empty_later_fields() {
    let mut e = ready();
    let o = open(&mut e, BUYER, USDC);
    assert_eq!((o.checkout_url.clone(), o.attempt, o.failures, o.lp_fault), (None, 0, 0, false));
    assert!(o.first_failure_at.is_none() && o.last_failure_at.is_none() && o.release_at.is_none());
    let v = serde_json::to_value(&o).unwrap();
    assert!(v["ghs_amount_pesewas"].is_string());
}
