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
    e
}

fn ready() -> Escrow {
    with_lp("paystack", 200 * USDC, 300 * USDC)
}

fn open(e: &mut Escrow, buyer: &str, usdc: u128) -> crate::Order {
    at(buyer, near(100), T0);
    e.open_buy_order(acc(LP), U128(usdc))
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
    assert_eq!(o.ghs_amount_pesewas, 15_000);
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
                   "ghs_amount_pesewas": 15000, "reference": format!("opr-{tag}-0"),
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
    e.open_buy_order(acc(LP), U128(USDC));
}

#[test]
fn open_refunds_storage_excess_to_buyer() {
    let mut e = ready();
    at(BUYER, NearToken::from_near(1), T0);
    e.open_buy_order(acc(LP), U128(USDC));
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
    e.open_buy_order(acc("nobody.testnet"), U128(USDC));
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
    e.buyer_released.insert(acc(BUYER), 3); // not new, so only the quote limit applies
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
    e.buyer_released.insert(acc(BUYER), 3); // new_buyer_orders = 3
    let o = open(&mut e, BUYER, 120 * USDC);
    assert!(!o.new_buyer);
    assert_eq!(e.get_lp(acc(LP)).unwrap().new_buyer_open.0, 0);
}

#[test]
fn buyer_with_fewer_released_orders_is_still_new() {
    let mut e = ready();
    e.buyer_released.insert(acc(BUYER), 2);
    assert!(open(&mut e, BUYER, USDC).new_buyer);
    assert_eq!(e.get_buyer_released(acc(BUYER)), 2);
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
    e.buyer_released.insert(acc(BUYER), 3);
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
