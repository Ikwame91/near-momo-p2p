//! M1.7: the books check (SPEC inv. 6) as one test across all buy flows. The test keeps its own
//! model of the contract's USDC balance (tokens in via ft_on_transfer, out via successful
//! ft_transfer) and after every step checks: balance >= sum over LPs of bond + liquidity + locked,
//! plus every claimable balance. ">=", not "=": anyone can send stray tokens to the escrow.

use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, Quote, SecretRef, Verdict, VerifierOutput, VerifierResult};
use near_sdk::json_types::U128;
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{testing_env, NearToken, PromiseError};

const TOKEN: &str = "usdc.testnet";
const LPS: [&str; 2] = ["lp1.testnet", "lp2.testnet"];
const BUYERS: [&str; 3] = ["ama.testnet", "kofi.testnet", "esi.testnet"];
const USDC: u128 = 1_000_000;
const MIN: u64 = 60_000_000_000;
const T0: u64 = 1_800_000_000_000_000_000;

struct Books {
    balance: u128,
}

fn at(who: &str, time: u64, yocto: u128) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(NearToken::from_yoctonear(yocto))
        .block_timestamp(time)
        .build());
}

fn owed(e: &Escrow) -> u128 {
    let lps: u128 = LPS.iter().filter_map(|l| e.get_lp(acc(l))).map(|l| l.bond.0 + l.liquidity.0 + l.locked.0).sum();
    let claimable: u128 = BUYERS.iter().chain(LPS.iter()).map(|a| e.get_claimable(acc(a)).0).sum();
    lps + claimable
}

fn check(e: &Escrow, b: &Books, step: &str) {
    let o = owed(e);
    assert!(b.balance >= o, "inv. 6 broken after {step}: balance {} < owed {o}", b.balance);
    for l in LPS {
        if let Some(lp) = e.get_lp(acc(l)) {
            assert!(lp.locked.0 <= lp.bond.0, "inv. 5 broken after {step} for {l}");
        }
    }
}

fn deposit(e: &mut Escrow, b: &mut Books, lp: &str, amount: u128, msg: &str) {
    at(TOKEN, T0, 0);
    e.ft_on_transfer(acc(lp), U128(amount), msg.into()).detach();
    b.balance += amount;
}

fn verdict(e: &mut Escrow, id: u64, time: u64, v: Option<(Verdict, &str)>) {
    at("anyone.testnet", time, 0);
    let attempt = e.begin_claim(id);
    at(ME, time, 0);
    let lp_fp = e.get_lp(e.get_order(id).unwrap().lp).unwrap().key_fp.unwrap();
    let r = match v {
        None => VerifierResult::RunFailed,
        Some((verdict, reason)) => VerifierResult::Output(VerifierOutput {
            verdict,
            reference: e.get_order(id).unwrap().reference,
            key_fp: lp_fp,
            reason: reason.into(),
        }),
    };
    e.on_verdict(id, attempt, r);
}

#[test]
fn books_balance_covers_everything_owed_across_all_flows() {
    at(ME, T0, 0);
    let mut e = init_with(verifier(), params());
    let mut b = Books { balance: 0 };
    for l in LPS {
        at(l, T0, 100_000_000_000_000_000_000_000);
        e.register_lp(
            "paystack".into(),
            SecretRef { profile: "sec".into(), account_id: acc(l) },
            Quote {
                rate_pesewas_per_usdc: U128(1_500),
                min_usdc: U128(USDC),
                max_usdc: U128(200 * USDC),
                new_buyer_exposure_max: U128(100 * USDC),
            },
        );
        deposit(&mut e, &mut b, l, 50 * USDC, "bond");
        deposit(&mut e, &mut b, l, 100 * USDC, "liquidity");
        e.lps.get_mut(&acc(l)).unwrap().key_fp = Some("ab".repeat(32));
    }
    check(&e, &b, "deposits");

    // Stray tokens sent straight to the escrow (not via ft_on_transfer): balance > owed is fine.
    b.balance += 7 * USDC;
    check(&e, &b, "stray tokens");

    // Orders: 0 (lp1, ama) released; 1 (lp1, kofi) slashed; 2 (lp2, esi) expired;
    // 3 (lp2, ama) NOT_PAID then released.
    let open = |e: &mut Escrow, buyer: &str, lp: &str, usdc: u128| {
        at(buyer, T0, 100_000_000_000_000_000_000_000);
        e.open_buy_order(acc(lp), U128(usdc), None).id
    };
    let o0 = open(&mut e, BUYERS[0], LPS[0], 10 * USDC);
    let o1 = open(&mut e, BUYERS[1], LPS[0], 20 * USDC);
    let o2 = open(&mut e, BUYERS[2], LPS[1], 15 * USDC);
    let o3 = open(&mut e, BUYERS[0], LPS[1], 5 * USDC);
    check(&e, &b, "orders opened");

    verdict(&mut e, o0, T0 + MIN, Some((Verdict::Paid, "ok")));
    check(&e, &b, "release o0 (claimable)");

    // Payout of ama's claimable: the transfer fails once (re-credited), then succeeds.
    at("anyone.testnet", T0 + MIN, 0);
    let _ = e.withdraw_claimable(acc(BUYERS[0]));
    check(&e, &b, "withdraw_claimable debited (transfer in flight)");
    at(ME, T0 + MIN, 0);
    e.on_payout(acc(BUYERS[0]), U128(10 * USDC), Err(PromiseError::Failed));
    check(&e, &b, "failed payout re-credited");
    at("anyone.testnet", T0 + MIN, 0);
    let _ = e.withdraw_claimable(acc(BUYERS[0]));
    b.balance -= 10 * USDC; // the transfer succeeded
    check(&e, &b, "payout succeeded");

    verdict(&mut e, o1, T0 + MIN, Some((Verdict::Unverifiable, "auth_error")));
    verdict(&mut e, o1, T0 + 4 * MIN, Some((Verdict::Unverifiable, "auth_error")));
    check(&e, &b, "slash o1 (bond -> claimable)");

    verdict(&mut e, o3, T0 + MIN, Some((Verdict::NotPaid, "not_found")));
    verdict(&mut e, o3, T0 + 2 * MIN, Some((Verdict::Paid, "ok")));
    check(&e, &b, "o3 NOT_PAID then released");

    at("anyone.testnet", T0 + 13 * MIN, 0);
    e.expire_order(o2);
    check(&e, &b, "expire o2");

    // LP withdraws free liquidity (succeeds) and, with no open orders, its bond.
    at(LPS[1], T0 + 13 * MIN, 1);
    let _ = e.withdraw_liquidity(U128(30 * USDC));
    b.balance -= 30 * USDC;
    check(&e, &b, "withdraw liquidity");
    at(LPS[1], T0 + 13 * MIN, 1);
    let _ = e.withdraw_bond(U128(50 * USDC));
    b.balance -= 50 * USDC;
    check(&e, &b, "withdraw bond");

    // Everyone owed is paid out; what remains is exactly what LPs still hold + the stray tokens.
    for a in BUYERS {
        let c = e.get_claimable(acc(a)).0;
        if c > 0 {
            at("anyone.testnet", T0 + 13 * MIN, 0);
            let _ = e.withdraw_claimable(acc(a));
            b.balance -= c;
        }
    }
    check(&e, &b, "all claimables paid");
    assert_eq!(b.balance - owed(&e), 7 * USDC, "only the stray tokens are unowed");
}
