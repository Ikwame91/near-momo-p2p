//! M1.3 tests: NEP-297 events, provider limit, secret lock (inv. 4), bond rule (inv. 5),
//! pause / resume (inv. 14).

use crate::lp::MAX_PROFILE_LEN;
use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, Quote, SecretRef};
use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, Value};
use near_sdk::test_utils::{get_logs, VMContextBuilder};
use near_sdk::{env, testing_env, NearToken};

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp.testnet";
const USDC: u128 = 1_000_000;

fn as_(who: &str, yocto: u128) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(NearToken::from_yoctonear(yocto))
        .build());
}

fn quote() -> Quote {
    Quote {
        rate_pesewas_per_usdc: U128(1_500),
        min_usdc: U128(USDC),
        max_usdc: U128(50 * USDC),
        new_buyer_exposure_max: U128(100 * USDC),
    }
}

fn sref(profile: &str) -> SecretRef {
    SecretRef { profile: profile.into(), account_id: acc(LP) }
}

fn deployed() -> Escrow {
    as_(ME, 0);
    init_with(verifier(), params())
}

fn registered(provider: &str) -> Escrow {
    let mut e = deployed();
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(LP))
        .attached_deposit(NearToken::from_millinear(100))
        .build());
    e.register_lp(provider.into(), sref("paystack"), quote());
    e
}

fn deposit(e: &mut Escrow, amount: u128, msg: &str) {
    as_(TOKEN, 0);
    e.ft_on_transfer(acc(LP), U128(amount), msg.into()).detach();
}

/// Buy-side LP with bond 10 USDC, liquidity 20 USDC and a fingerprinted key.
fn ready() -> Escrow {
    let mut e = registered("paystack");
    deposit(&mut e, 10 * USDC, "bond");
    deposit(&mut e, 20 * USDC, "liquidity");
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some("ab".repeat(32));
    e
}

/// All NEP-297 events in the current logs as (event, data).
fn events() -> Vec<(String, Value)> {
    get_logs()
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .map(|j| {
            let v: Value = serde_json::from_str(j).unwrap();
            assert_eq!(v["standard"], "openramp");
            assert_eq!(v["version"], "1.0.0");
            (v["event"].as_str().unwrap().to_string(), v["data"].clone())
        })
        .collect()
}

fn last_event() -> (String, Value) {
    events().pop().expect("an event")
}

// ---- events (no personal data) ----

#[test]
fn register_emits_event_without_secret_profile() {
    let _e = registered("paystack");
    let (name, data) = last_event();
    assert_eq!(name, "lp_registered");
    assert_eq!(data["lp"], LP);
    assert_eq!(data["provider"], "paystack");
    let raw = get_logs().join("\n");
    assert!(!raw.contains("\"profile\""), "secret profile must not be logged");
}

#[test]
fn deposit_emits_event() {
    let mut e = registered("paystack");
    deposit(&mut e, 7 * USDC, "bond");
    let (name, data) = last_event();
    assert_eq!(name, "deposit");
    assert_eq!((data["lp"].as_str(), data["pot"].as_str(), data["amount"].as_str()), (Some(LP), Some("Bond"), Some("7000000")));
}

#[test]
fn withdraw_and_recredit_emit_events() {
    let mut e = ready();
    as_(LP, 1);
    let _ = e.withdraw_liquidity(U128(USDC));
    assert_eq!(last_event().0, "withdraw_requested");
    as_(ME, 0);
    e.on_withdraw(acc(LP), crate::Pot::Liquidity, U128(USDC), Err(near_sdk::PromiseError::Failed));
    let (name, data) = last_event();
    assert_eq!(name, "withdraw_recredited");
    assert_eq!(data["amount"], "1000000");
}

#[test]
fn successful_withdraw_callback_emits_nothing() {
    let mut e = ready();
    as_(ME, 0);
    assert!(e.on_withdraw(acc(LP), crate::Pot::Liquidity, U128(USDC), Ok(())));
    assert!(events().is_empty());
}

#[test]
fn quote_update_emits_event() {
    let mut e = registered("paystack");
    let mut q = quote();
    q.rate_pesewas_per_usdc = U128(1_650);
    as_(LP, 1);
    e.update_quote(q);
    let (name, data) = last_event();
    assert_eq!(name, "quote_updated");
    assert_eq!(data["rate_pesewas_per_usdc"], "1650");
}

// ---- provider limit (D-017) ----

#[test]
fn pawapay_provider_is_accepted() {
    let e = registered("pawapay");
    assert_eq!(e.get_lp(acc(LP)).unwrap().provider, "pawapay");
}

#[test]
#[should_panic(expected = "provider must be \"paystack\" or \"pawapay\"")]
fn provider_is_case_sensitive() {
    registered("Paystack");
}

#[test]
#[should_panic(expected = "provider must be \"paystack\" or \"pawapay\"")]
fn unknown_provider_fails() {
    registered("flutterwave");
}

// ---- secret lock (inv. 4) ----

#[test]
fn set_secret_ref_with_no_orders_clears_key_fp_and_emits() {
    let mut e = ready();
    as_(LP, 1);
    e.set_secret_ref(sref("paystack-v2"));
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!(lp.secret_ref.profile, "paystack-v2");
    assert!(lp.key_fp.is_none(), "new key must be fingerprinted before new orders");
    let (name, data) = last_event();
    assert_eq!(name, "secret_ref_changed");
    assert!(data.get("profile").is_none());
}

#[test]
#[should_panic(expected = "secret is locked while orders are open")]
fn set_secret_ref_with_open_order_fails() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), USDC);
    as_(LP, 1);
    e.set_secret_ref(sref("other"));
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn set_secret_ref_without_one_yocto_fails() {
    let mut e = ready();
    as_(LP, 0);
    e.set_secret_ref(sref("other"));
}

#[test]
#[should_panic(expected = "secret_ref must belong to the registering LP")]
fn set_secret_ref_to_someone_elses_secret_fails() {
    let mut e = ready();
    as_(LP, 1);
    e.set_secret_ref(SecretRef { profile: "paystack".into(), account_id: acc("other-lp.testnet") });
}

#[test]
#[should_panic(expected = "secret profile must be 1-64 chars")]
fn set_secret_ref_with_too_long_profile_fails() {
    let mut e = ready();
    as_(LP, 1);
    e.set_secret_ref(sref(&"x".repeat(MAX_PROFILE_LEN + 1)));
}

#[test]
fn longest_profile_and_key_fp_fit_in_prepaid_storage() {
    // Registered with an 8-char profile; growing to 64 chars plus a key_fp must stay within the
    // bytes reserved at registration (profile room 56 + key_fp 68).
    let mut e = registered("paystack");
    as_(LP, 1); // a new test context resets the mock's storage counter, so measure inside it
    e.lps.flush();
    let before = env::storage_usage();
    e.set_secret_ref(sref(&"x".repeat(MAX_PROFILE_LEN)));
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some("ab".repeat(32));
    e.lps.flush();
    let grown = env::storage_usage() - before;
    let reserved = (MAX_PROFILE_LEN - "paystack".len()) as u64 + crate::lp::KEY_FP_EXTRA_BYTES;
    assert!(grown <= reserved, "grew {grown} bytes, reserved {reserved}");
}

#[test]
#[should_panic(expected = "bond is locked while orders are open")]
fn withdraw_bond_with_real_open_order_fails() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), USDC);
    as_(LP, 1);
    let _ = e.withdraw_bond(U128(USDC));
}

// ---- bond rule (inv. 5) and locks ----

#[test]
fn lock_moves_liquidity_to_locked_and_counts_order() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), 4 * USDC);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.liquidity.0, lp.locked.0, lp.open_orders), (16 * USDC, 4 * USDC, 1));
}

#[test]
fn lock_up_to_exactly_the_bond_is_allowed() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), 6 * USDC);
    e.lock_for_buy(&acc(LP), 4 * USDC);
    assert_eq!(e.get_lp(acc(LP)).unwrap().locked.0, 10 * USDC);
}

#[test]
#[should_panic(expected = "bond does not cover this order")]
fn lock_beyond_the_bond_fails() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), 10 * USDC);
    e.lock_for_buy(&acc(LP), 1);
}

#[test]
#[should_panic(expected = "not enough LP liquidity")]
fn lock_beyond_liquidity_fails() {
    let mut e = ready();
    deposit(&mut e, 100 * USDC, "bond"); // bond 110, liquidity 20
    e.lock_for_buy(&acc(LP), 21 * USDC);
}

#[test]
#[should_panic(expected = "LP does not serve buy orders")]
fn sell_side_lp_cannot_lock_for_buy() {
    let mut e = registered("pawapay");
    deposit(&mut e, 10 * USDC, "bond");
    deposit(&mut e, 10 * USDC, "liquidity");
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some("ab".repeat(32));
    e.lock_for_buy(&acc(LP), USDC);
}

#[test]
#[should_panic(expected = "LP key not fingerprinted yet")]
fn lock_without_key_fp_fails() {
    let mut e = ready();
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = None;
    e.lock_for_buy(&acc(LP), USDC);
}

#[test]
fn unlock_back_to_liquidity_restores_balances() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), 4 * USDC);
    e.unlock(&acc(LP), 4 * USDC, true);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.liquidity.0, lp.locked.0, lp.open_orders), (20 * USDC, 0, 0));
}

#[test]
fn unlock_released_leaves_the_lp() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), 4 * USDC);
    e.unlock(&acc(LP), 4 * USDC, false);
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.liquidity.0, lp.locked.0, lp.open_orders), (16 * USDC, 0, 0));
}

#[test]
#[should_panic(expected = "nothing locked")]
fn unlock_more_than_locked_fails() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), USDC);
    e.unlock(&acc(LP), 2 * USDC, true);
}

// ---- pause / resume (inv. 14, T28, T34) ----

#[test]
#[should_panic(expected = "LP is paused")]
fn paused_lp_cannot_lock_new_orders() {
    let mut e = ready();
    as_(LP, 1);
    e.pause_lp();
    e.lock_for_buy(&acc(LP), USDC);
}

#[test]
fn paused_lp_can_still_close_open_orders_and_resume() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), USDC);
    as_(LP, 1);
    e.pause_lp();
    assert_eq!(last_event().0, "lp_paused");
    e.unlock(&acc(LP), USDC, true); // open order runs to the end
    as_(LP, 1);
    e.resume_lp();
    assert_eq!(last_event().0, "lp_resumed");
    e.lock_for_buy(&acc(LP), USDC);
    assert_eq!(e.get_lp(acc(LP)).unwrap().open_orders, 1);
}

#[test]
fn pausing_twice_emits_one_event() {
    let mut e = ready();
    as_(LP, 1);
    e.pause_lp();
    as_(LP, 1);
    e.pause_lp();
    assert!(events().is_empty(), "second pause is a no-op");
    assert!(e.get_lp(acc(LP)).unwrap().paused);
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn pause_without_one_yocto_fails() {
    let mut e = ready();
    as_(LP, 0);
    e.pause_lp();
}

#[test]
#[should_panic(expected = "not a registered LP")]
fn pause_by_stranger_fails() {
    let mut e = ready();
    as_("stranger.testnet", 1);
    e.pause_lp();
}
