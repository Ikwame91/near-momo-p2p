//! M1.3 tests: NEP-297 events, provider limit, secret lock (inv. 4), bond rule (inv. 5),
//! pause / resume (inv. 14).

use crate::lp::MAX_PROFILE_LEN;
use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, Quote, SecretRef};
use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, json, Value};
use near_sdk::test_utils::{get_logs, VMContextBuilder};
use near_sdk::{env, testing_env, NearToken};

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp.testnet";
const USDC: u128 = 1_000_000;
/// Distinctive profile name, so a leak into the logs can be detected by value (not just by key).
const SECRET: &str = "lp-secret-7f3a";

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
    e.register_lp(provider.into(), sref(SECRET), quote());
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
    assert_eq!(last_event(), ("lp_registered".into(), json!({"lp": LP, "provider": "paystack"})));
    assert!(!get_logs().join("\n").contains(SECRET), "secret profile must not be logged");
}

#[test]
fn deposit_emits_event() {
    let mut e = registered("paystack");
    deposit(&mut e, 7 * USDC, "bond");
    assert_eq!(last_event(), ("deposit".into(), json!({"lp": LP, "pot": "bond", "amount": "7000000"})));
}

#[test]
fn withdraw_and_recredit_emit_events() {
    let mut e = ready();
    as_(LP, 1);
    let _ = e.withdraw_liquidity(U128(USDC));
    assert_eq!(
        last_event(),
        ("withdraw_requested".into(), json!({"lp": LP, "pot": "liquidity", "amount": "1000000"}))
    );
    as_(ME, 0);
    e.on_withdraw(acc(LP), crate::Pot::Liquidity, U128(USDC), Err(near_sdk::PromiseError::Failed));
    assert_eq!(
        last_event(),
        ("withdraw_recredited".into(), json!({"lp": LP, "pot": "liquidity", "amount": "1000000"}))
    );
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
    let q = Quote {
        rate_pesewas_per_usdc: U128(1_650),
        min_usdc: U128(2 * USDC),
        max_usdc: U128(40 * USDC),
        new_buyer_exposure_max: U128(90 * USDC),
    };
    as_(LP, 1);
    e.update_quote(q);
    assert_eq!(
        last_event(),
        (
            "quote_updated".into(),
            json!({"lp": LP, "rate_pesewas_per_usdc": "1650", "min_usdc": "2000000",
                   "max_usdc": "40000000", "new_buyer_exposure_max": "90000000"})
        )
    );
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
    assert_eq!(last_event(), ("secret_ref_changed".into(), json!({"lp": LP})));
    let logs = get_logs().join("\n");
    assert!(!logs.contains("paystack-v2") && !logs.contains(SECRET), "profile names must not be logged");
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
#[should_panic(expected = "secret_ref must belong to the caller")]
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
    // Registered with a 14-char profile; growing to 64 chars plus a key_fp must stay within the
    // bytes reserved at registration (profile room 50 + key_fp 68).
    let mut e = registered("paystack");
    as_(LP, 1); // a new test context resets the mock's storage counter, so measure inside it
    e.lps.flush();
    let before = env::storage_usage();
    e.set_secret_ref(sref(&"x".repeat(MAX_PROFILE_LEN)));
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some("ab".repeat(32));
    e.lps.flush();
    let grown = env::storage_usage() - before;
    let reserved = (MAX_PROFILE_LEN - SECRET.len()) as u64 + crate::lp::KEY_FP_EXTRA_BYTES;
    assert_eq!(grown, reserved, "growth must exactly use the reserved room");
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
#[should_panic(expected = "unlock exceeds locked")]
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
    assert_eq!(last_event(), ("lp_paused".into(), json!({"lp": LP})));
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

// ---- review fix-ups (PR #9) ----

#[test]
fn bond_withdraw_event_names_bond_pot() {
    let mut e = ready();
    as_(LP, 1);
    let _ = e.withdraw_bond(U128(USDC));
    assert_eq!(last_event(), ("withdraw_requested".into(), json!({"lp": LP, "pot": "bond", "amount": "1000000"})));
}

/// Registration cost read from the refund, for an LP account `who` with `profile`.
fn cost_for(e: &mut Escrow, who: &str, profile: &str) -> NearToken {
    let paid = NearToken::from_near(1);
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(paid)
        .build());
    e.register_lp("paystack".into(), SecretRef { profile: profile.into(), account_id: acc(who) }, quote());
    let refund: Vec<NearToken> = near_sdk::test_utils::get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id.as_str() == who)
        .flat_map(|r| r.actions)
        .filter_map(|a| if let near_sdk::mock::MockAction::Transfer { deposit, .. } = a { Some(deposit) } else { None })
        .collect();
    assert_eq!(refund.len(), 1);
    paid.saturating_sub(refund[0])
}

#[test]
fn registration_cost_is_the_same_for_short_and_longest_profile() {
    // Proves the 64-char profile room is charged: a 1-char profile pays as much as a 64-char one.
    // Two accounts with ids of the same length (mocked storage persists within a test).
    let mut e = deployed();
    let short = cost_for(&mut e, "la.testnet", "p");
    let long = cost_for(&mut e, "lb.testnet", &"p".repeat(MAX_PROFILE_LEN));
    assert_eq!(short, long);
}

#[test]
fn register_with_exactly_64_char_profile_is_accepted() {
    let mut e = deployed();
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(LP))
        .attached_deposit(NearToken::from_millinear(100))
        .build());
    e.register_lp("paystack".into(), sref(&"x".repeat(MAX_PROFILE_LEN)), quote());
    assert_eq!(e.get_lp(acc(LP)).unwrap().secret_ref.profile.len(), MAX_PROFILE_LEN);
}

#[test]
#[should_panic(expected = "secret profile must be 1-64 chars of A-Z a-z 0-9 - _")]
fn profile_with_free_text_is_rejected() {
    let mut e = deployed();
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(LP))
        .attached_deposit(NearToken::from_millinear(100))
        .build());
    e.register_lp("paystack".into(), sref("+233 55 123 4987"), quote());
}

#[test]
#[should_panic(expected = "secret profile must be 1-64 chars")]
fn set_secret_ref_with_empty_profile_fails() {
    let mut e = ready();
    as_(LP, 1);
    e.set_secret_ref(sref(""));
}

#[test]
#[should_panic(expected = "not a registered LP")]
fn set_secret_ref_by_unregistered_caller_fails() {
    let mut e = ready();
    as_("stranger.testnet", 1);
    e.set_secret_ref(SecretRef { profile: "x".into(), account_id: acc("stranger.testnet") });
}

#[test]
fn set_secret_ref_to_the_same_ref_clears_key_fp() {
    // How an LP re-fingerprints after rotating the key inside the same OutLayer secret.
    let mut e = ready();
    as_(LP, 1);
    e.set_secret_ref(sref(SECRET));
    assert!(e.get_lp(acc(LP)).unwrap().key_fp.is_none());
    assert_eq!(last_event().0, "secret_ref_changed");
}

#[test]
#[should_panic(expected = "amount must be > 0")]
fn lock_zero_fails() {
    let mut e = ready();
    e.lock_for_buy(&acc(LP), 0);
}

#[test]
#[should_panic(expected = "amount must be > 0")]
fn unlock_zero_fails() {
    // Would otherwise drop open_orders while USDC stays locked, unlocking withdraw_bond (inv. 4).
    let mut e = ready();
    e.lock_for_buy(&acc(LP), USDC);
    e.unlock(&acc(LP), 0, true);
}

#[test]
#[should_panic(expected = "no open orders")]
fn unlock_with_no_open_orders_fails() {
    let mut e = ready();
    e.unlock(&acc(LP), USDC, true);
}
