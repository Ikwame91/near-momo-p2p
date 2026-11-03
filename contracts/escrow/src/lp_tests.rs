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
    as_(LP, yocto(0));
    e.update_quote(q.clone());
    assert_eq!(e.get_lp(acc(LP)).unwrap().quote, q);
}

#[test]
#[should_panic(expected = "not a registered LP")]
fn update_quote_by_non_lp_fails() {
    let mut e = with_lp();
    as_("stranger.testnet", yocto(0));
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
