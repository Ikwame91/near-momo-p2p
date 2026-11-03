//! Unit tests for LP registration, deposits and withdrawals (PLAN M1.2).

use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, Pot, Quote, SecretRef};
use near_sdk::json_types::U128;
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{testing_env, NearToken, PromiseError};

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp.testnet";
const USDC: u128 = 1_000_000;

fn as_(who: &str, deposit: NearToken) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(deposit)
        .build());
}

fn yocto(n: u128) -> NearToken {
    NearToken::from_yoctonear(n)
}

fn quote() -> Quote {
    Quote {
        rate_pesewas_per_usdc: U128(1_500),
        min_usdc: U128(USDC),
        max_usdc: U128(50 * USDC),
        new_buyer_exposure_max: U128(100 * USDC),
    }
}

fn secret_ref() -> SecretRef {
    SecretRef { profile: "paystack".into(), account_id: acc(LP) }
}

fn deployed() -> Escrow {
    as_(ME, yocto(0));
    init_with(verifier(), params())
}

/// Escrow with LP registered (storage paid generously).
fn with_lp() -> Escrow {
    let mut e = deployed();
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("paystack".into(), secret_ref(), quote());
    e
}

fn deposit(e: &mut Escrow, from: &str, amount: u128, msg: &str) {
    as_(TOKEN, yocto(0));
    e.ft_on_transfer(acc(from), U128(amount), msg.into()).detach();
}

// ---- registration ----

#[test]
fn register_stores_lp_with_empty_balances() {
    let e = with_lp();
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!(lp.provider, "paystack");
    assert_eq!(lp.secret_ref, secret_ref());
    assert_eq!(lp.quote, quote());
    assert_eq!((lp.bond.0, lp.liquidity.0, lp.locked.0, lp.open_orders, lp.paused), (0, 0, 0, 0, false));
}

#[test]
#[should_panic(expected = "for storage")]
fn register_without_storage_deposit_fails() {
    let mut e = deployed();
    as_(LP, yocto(0));
    e.register_lp("paystack".into(), secret_ref(), quote());
}

#[test]
#[should_panic(expected = "already registered")]
fn register_twice_fails() {
    let mut e = with_lp();
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("paystack".into(), secret_ref(), quote());
}

#[test]
#[should_panic(expected = "min must be <= max")]
fn register_with_min_above_max_fails() {
    let mut e = deployed();
    let mut q = quote();
    q.min_usdc = U128(q.max_usdc.0 + 1);
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("paystack".into(), secret_ref(), q);
}

#[test]
#[should_panic(expected = "rate must be > 0")]
fn register_with_zero_rate_fails() {
    let mut e = deployed();
    let mut q = quote();
    q.rate_pesewas_per_usdc = U128(0);
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("paystack".into(), secret_ref(), q);
}

#[test]
#[should_panic(expected = "secret profile must be 1-64 chars")]
fn register_with_empty_secret_profile_fails() {
    let mut e = deployed();
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("paystack".into(), SecretRef { profile: "".into(), account_id: acc(LP) }, quote());
}

#[test]
#[should_panic(expected = "secret_ref must belong to the registering LP")]
fn register_with_someone_elses_secret_fails() {
    let mut e = deployed();
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("paystack".into(), SecretRef { profile: "paystack".into(), account_id: acc("other-lp.testnet") }, quote());
}

#[test]
fn update_quote_changes_quote() {
    let mut e = with_lp();
    let mut q = quote();
    q.rate_pesewas_per_usdc = U128(1_600);
    as_(LP, yocto(1));
    e.update_quote(q.clone());
    assert_eq!(e.get_lp(acc(LP)).unwrap().quote, q);
}

#[test]
#[should_panic(expected = "not a registered LP")]
fn update_quote_by_non_lp_fails() {
    let mut e = with_lp();
    as_("stranger.testnet", yocto(1));
    e.update_quote(quote());
}

// ---- deposits (ft_on_transfer) ----

#[test]
fn deposits_credit_bond_and_liquidity() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "bond");
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    deposit(&mut e, LP, 5 * USDC, "bond");
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.bond.0, lp.liquidity.0), (25 * USDC, 30 * USDC));
}

#[test]
#[should_panic(expected = "only the configured token is accepted")]
fn deposit_from_other_token_fails() {
    let mut e = with_lp();
    as_("fake-usdc.testnet", yocto(0));
    e.ft_on_transfer(acc(LP), U128(USDC), "bond".into()).detach();
}

#[test]
#[should_panic(expected = "only the configured token is accepted")]
fn deposit_called_directly_by_lp_fails() {
    let mut e = with_lp();
    as_(LP, yocto(0));
    e.ft_on_transfer(acc(LP), U128(USDC), "bond".into()).detach();
}

#[test]
#[should_panic(expected = "msg must be")]
fn deposit_with_unknown_msg_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, USDC, "gift");
}

#[test]
#[should_panic(expected = "not a registered LP")]
fn deposit_from_unregistered_sender_fails() {
    let mut e = with_lp();
    deposit(&mut e, "stranger.testnet", USDC, "liquidity");
}

#[test]
#[should_panic(expected = "amount must be > 0")]
fn zero_deposit_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 0, "bond");
}

// ---- withdrawals ----

#[test]
fn withdraw_liquidity_debits_immediately() {
    let mut e = with_lp();
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    as_(LP, yocto(1));
    let _ = e.withdraw_liquidity(U128(10 * USDC));
    assert_eq!(e.get_lp(acc(LP)).unwrap().liquidity.0, 20 * USDC);
}

#[test]
#[should_panic(expected = "amount exceeds available balance")]
fn withdraw_more_liquidity_than_available_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    as_(LP, yocto(1));
    let _ = e.withdraw_liquidity(U128(30 * USDC + 1));
}

#[test]
#[should_panic(expected = "amount exceeds available balance")]
fn withdraw_bond_cannot_take_liquidity() {
    let mut e = with_lp();
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    as_(LP, yocto(1));
    let _ = e.withdraw_bond(U128(1));
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn withdraw_without_one_yocto_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    as_(LP, yocto(0));
    let _ = e.withdraw_liquidity(U128(USDC));
}

#[test]
#[should_panic(expected = "not a registered LP")]
fn withdraw_by_stranger_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    as_("stranger.testnet", yocto(1));
    let _ = e.withdraw_liquidity(U128(USDC));
}

#[test]
fn withdraw_bond_with_no_open_orders_debits() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "bond");
    as_(LP, yocto(1));
    let _ = e.withdraw_bond(U128(20 * USDC));
    assert_eq!(e.get_lp(acc(LP)).unwrap().bond.0, 0);
}

#[test]
fn failed_token_transfer_recredits() {
    let mut e = with_lp();
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    as_(LP, yocto(1));
    let _ = e.withdraw_liquidity(U128(10 * USDC));
    as_(ME, yocto(0)); // callback comes from the contract itself
    assert!(!e.on_withdraw(acc(LP), Pot::Liquidity, U128(10 * USDC), Err(PromiseError::Failed)));
    assert_eq!(e.get_lp(acc(LP)).unwrap().liquidity.0, 30 * USDC);
}

#[test]
fn successful_token_transfer_keeps_debit() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "bond");
    as_(LP, yocto(1));
    let _ = e.withdraw_bond(U128(5 * USDC));
    as_(ME, yocto(0));
    assert!(e.on_withdraw(acc(LP), Pot::Bond, U128(5 * USDC), Ok(())));
    assert_eq!(e.get_lp(acc(LP)).unwrap().bond.0, 15 * USDC);
}

// ---- review fix-up (PR #7): receipts, gas, 1-yocto, edges ----

use crate::lp::{FT_TRANSFER_MIN_GAS, KEY_FP_EXTRA_BYTES, WITHDRAW_CALLBACK_GAS};
use near_sdk::mock::MockAction;
use near_sdk::test_utils::get_created_receipts;
use near_sdk::{env, PromiseOrValue};

/// (receiver, method, args JSON, deposit, gas, weight) of every function call created so far.
fn calls() -> Vec<(String, String, near_sdk::serde_json::Value, NearToken, near_sdk::Gas, u64)> {
    let mut out = vec![];
    for r in get_created_receipts() {
        for a in r.actions {
            if let MockAction::FunctionCallWeight { method_name, args, attached_deposit, prepaid_gas, gas_weight, .. } = a {
                out.push((
                    r.receiver_id.to_string(),
                    String::from_utf8(method_name).unwrap(),
                    near_sdk::serde_json::from_slice(&args).unwrap(),
                    attached_deposit,
                    prepaid_gas,
                    gas_weight.0,
                ));
            }
        }
    }
    out
}

fn transfers_to(who: &str) -> Vec<NearToken> {
    get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id.as_str() == who)
        .flat_map(|r| r.actions)
        .filter_map(|a| if let MockAction::Transfer { deposit, .. } = a { Some(deposit) } else { None })
        .collect()
}

#[test]
fn withdraw_creates_exact_transfer_and_callback() {
    let mut e = with_lp();
    deposit(&mut e, LP, 30 * USDC, "liquidity");
    as_(LP, yocto(1));
    let _ = e.withdraw_liquidity(U128(5 * USDC));
    let c = calls();
    let ft: Vec<_> = c.iter().filter(|x| x.1 == "ft_transfer").collect();
    let cb: Vec<_> = c.iter().filter(|x| x.1 == "on_withdraw").collect();
    assert_eq!(ft.len(), 1);
    assert_eq!(cb.len(), 1);
    let (to, _, args, dep, gas, weight) = ft[0];
    assert_eq!(to, TOKEN);
    assert_eq!(args["receiver_id"], LP);
    assert_eq!(args["amount"], (5 * USDC).to_string());
    assert_eq!(*dep, yocto(1));
    assert_eq!(*gas, FT_TRANSFER_MIN_GAS);
    assert_eq!(*weight, 1, "ft_transfer must get the unused gas");
    let (to, _, args, dep, gas, weight) = cb[0];
    assert_eq!(to, ME);
    assert_eq!(args["lp_id"], LP);
    assert_eq!(args["pot"], "Liquidity");
    assert_eq!(args["amount"], (5 * USDC).to_string());
    assert_eq!(*dep, yocto(0));
    assert_eq!(*gas, WITHDRAW_CALLBACK_GAS);
    assert_eq!(*weight, 0, "callback has a fixed budget");
}

#[test]
fn withdraw_bond_callback_names_bond_pot() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "bond");
    as_(LP, yocto(1));
    let _ = e.withdraw_bond(U128(USDC));
    let cb: Vec<_> = calls().into_iter().filter(|x| x.1 == "on_withdraw").collect();
    assert_eq!(cb[0].2["pot"], "Bond");
}

#[test]
fn ft_on_transfer_keeps_the_whole_amount() {
    let mut e = with_lp();
    as_(TOKEN, yocto(0));
    let r = e.ft_on_transfer(acc(LP), U128(USDC), "bond".into());
    assert!(matches!(r, PromiseOrValue::Value(U128(0))));
}

#[test]
#[should_panic(expected = "only the contract itself")]
fn on_withdraw_called_by_stranger_fails() {
    let mut e = with_lp();
    as_("stranger.testnet", yocto(0));
    e.on_withdraw(acc(LP), Pot::Liquidity, U128(USDC), Err(PromiseError::Failed));
}

#[test]
fn failed_bond_transfer_recredits_bond_not_liquidity() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "bond");
    as_(LP, yocto(1));
    let _ = e.withdraw_bond(U128(5 * USDC));
    as_(ME, yocto(0));
    assert!(!e.on_withdraw(acc(LP), Pot::Bond, U128(5 * USDC), Err(PromiseError::Failed)));
    let lp = e.get_lp(acc(LP)).unwrap();
    assert_eq!((lp.bond.0, lp.liquidity.0), (20 * USDC, 0));
}

#[test]
#[should_panic(expected = "bond is locked while orders are open")]
fn withdraw_bond_with_open_order_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "bond");
    e.lps.get_mut(&acc(LP)).unwrap().open_orders = 1; // orders arrive in M1.4
    as_(LP, yocto(1));
    let _ = e.withdraw_bond(U128(USDC));
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn update_quote_without_one_yocto_fails() {
    let mut e = with_lp();
    as_(LP, yocto(0));
    e.update_quote(quote());
}

#[test]
#[should_panic(expected = "min must be <= max")]
fn update_quote_with_invalid_quote_fails() {
    let mut e = with_lp();
    let mut q = quote();
    q.max_usdc = U128(0);
    as_(LP, yocto(1));
    e.update_quote(q);
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn withdraw_bond_without_one_yocto_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "bond");
    as_(LP, yocto(0));
    let _ = e.withdraw_bond(U128(USDC));
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn withdraw_with_two_yocto_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "liquidity");
    as_(LP, yocto(2));
    let _ = e.withdraw_liquidity(U128(USDC));
}

#[test]
#[should_panic(expected = "amount must be > 0")]
fn withdraw_zero_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, 20 * USDC, "liquidity");
    as_(LP, yocto(1));
    let _ = e.withdraw_liquidity(U128(0));
}

#[test]
#[should_panic(expected = "provider name must be 1-32 chars")]
fn register_with_empty_provider_fails() {
    let mut e = deployed();
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("".into(), secret_ref(), quote());
}

#[test]
#[should_panic(expected = "provider name must be 1-32 chars")]
fn register_with_long_provider_fails() {
    let mut e = deployed();
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("p".repeat(33), secret_ref(), quote());
}

#[test]
#[should_panic(expected = "secret profile must be 1-64 chars")]
fn register_with_long_profile_fails() {
    let mut e = deployed();
    as_(LP, NearToken::from_millinear(100));
    e.register_lp("paystack".into(), SecretRef { profile: "x".repeat(65), account_id: acc(LP) }, quote());
}

#[test]
#[should_panic(expected = "overflow")]
fn credit_overflow_fails() {
    let mut e = with_lp();
    deposit(&mut e, LP, u128::MAX, "bond");
    deposit(&mut e, LP, 1, "bond");
}

/// Storage price of one registration, read from the refund of an over-paid registration. Uses a
/// different LP account of the SAME name length ("lq.testnet" vs "lp.testnet"), because mocked
/// storage persists within a test and the storage size depends on the account id length.
fn registration_cost() -> NearToken {
    const PROBE: &str = "lq.testnet";
    assert_eq!(PROBE.len(), LP.len());
    let mut e = deployed();
    let paid = NearToken::from_near(1);
    as_(PROBE, paid);
    e.register_lp("paystack".into(), SecretRef { profile: "paystack".into(), account_id: acc(PROBE) }, quote());
    let refunds = transfers_to(PROBE);
    assert_eq!(refunds.len(), 1, "excess must be refunded once");
    paid.saturating_sub(refunds[0])
}

#[test]
fn registration_charges_record_plus_key_fp_room_and_refunds_the_rest() {
    let cost = registration_cost();
    // Includes the room reserved for key_fp, so the later fingerprint write is pre-paid.
    let min = env::storage_byte_cost().saturating_mul(KEY_FP_EXTRA_BYTES as u128);
    assert!(cost > min, "cost {cost} must exceed the key_fp reservation {min}");
}

#[test]
fn registration_with_exact_cost_succeeds_without_refund() {
    let cost = registration_cost();
    let mut e = deployed();
    as_(LP, cost);
    e.register_lp("paystack".into(), secret_ref(), quote());
    assert!(transfers_to(LP).is_empty());
    assert!(e.get_lp(acc(LP)).unwrap().key_fp.is_none());
}

#[test]
#[should_panic(expected = "for storage")]
fn registration_with_one_yocto_too_little_fails() {
    let cost = registration_cost();
    let mut e = deployed();
    as_(LP, cost.saturating_sub(yocto(1)));
    e.register_lp("paystack".into(), secret_ref(), quote());
}
