//! M1.9 (SPEC inv. 9, T26): nothing can change the escrow's settings after `new`: not the token,
//! the OutLayer account, the verifier, the parameters or the deploy tag. There is no owner (D-014)
//! and no setter. The token check in `ft_on_transfer` is tested in lp_tests (M1.2).

use crate::tests::{acc, init_with, params, verifier, ME};
use crate::{Escrow, Quote, SecretRef, Verdict, VerifierOutput, VerifierResult};
use near_sdk::json_types::U128;
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{testing_env, NearToken, PromiseError};
use std::collections::BTreeSet;

/// Every `pub fn` in the contract's source files (tests excluded). Pinned below, so that adding
/// any public method, a setter included, fails this test until someone reviews it.
const SOURCES: [&str; 5] = [
    include_str!("lib.rs"),
    include_str!("lp.rs"),
    include_str!("orders.rs"),
    include_str!("verdicts.rs"),
    include_str!("outlayer.rs"),
];

const PUBLIC_FNS: [&str; 50] = [
    "claim", "compute_deploy_tag", "count_orders_by_buyer", "count_orders_by_lp", "due_at",
    "expire_order", "finalize_release", "ft_on_transfer", "get_buyer_released", "get_claimable",
    "get_config", "get_lp", "get_order", "ghs_amount_pesewas", "last_claim", "list_orders_by_buyer",
    "list_orders_by_lp", "new", "on_checkout_result", "on_fingerprint_result", "on_payout",
    "on_verdict", "on_verdict_result", "on_withdraw", "open_buy_order", "parse_verify", "pause_lp",
    "register_lp", "request_checkout", "request_fingerprint", "resume_lp", "set_secret_ref", "slash",
    "slash_due", "update_quote", "withdraw_bond", "withdraw_claimable", "withdraw_liquidity",
    // pub(crate): internal money helpers, not callable from outside the contract.
    "begin_claim", "apply_verdict", "lock_for_buy", "unlock", "pay_out", "slash_lp", "slash_if_due",
    // test helpers in lib.rs `mod tests`.
    "init_with", "acc", "ctx", "params", "verifier",
];

fn public_fns() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for src in SOURCES {
        let mut rest = src;
        // the earliest of either form, so neither hides the other
        while let Some(i) = [rest.find("pub fn "), rest.find("pub(crate) fn ")].into_iter().flatten().min() {
            let after = &rest[i..];
            let start = after.find("fn ").unwrap() + 3;
            let name: String = after[start..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            out.insert(name);
            rest = &after[start..];
        }
    }
    out
}

#[test]
fn the_public_method_list_is_pinned_and_has_no_config_setter() {
    let found = public_fns();
    let pinned: BTreeSet<String> = PUBLIC_FNS.iter().map(|s| s.to_string()).collect();
    let added: Vec<_> = found.difference(&pinned).collect();
    let removed: Vec<_> = pinned.difference(&found).collect();
    assert!(added.is_empty() && removed.is_empty(), "public methods changed: added {added:?}, removed {removed:?}. Review: none may change token, OutLayer, verifier or params");
    // The only "change something" methods are an LP's own settings, guarded by 1 yoctoNEAR (D-016).
    let setters: Vec<_> = found
        .iter()
        .filter(|n| ["set_", "update_", "change_", "configure", "upgrade", "migrate", "owner"].iter().any(|p| n.contains(p)))
        .cloned()
        .collect();
    assert_eq!(setters, vec!["set_secret_ref".to_string(), "update_quote".to_string()]);
}

#[test]
fn no_code_writes_the_settings_after_new() {
    // The settings are only ever set in `new`'s struct literal. Flag `self.<field> = …`,
    // `self.<field>.<sub> = …` (nested), and mutating calls such as `self.<field>.push(…)`.
    for field in ["token", "outlayer", "verifier", "params", "deploy_tag"] {
        let pat = format!("self.{field}");
        for src in SOURCES {
            for (i, _) in src.match_indices(&pat) {
                let rest = &src[i + pat.len()..];
                let line = rest.lines().next().unwrap_or("");
                // skip a longer name that merely starts with the field (e.g. self.params_x)
                if line.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let path_end = line.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')).unwrap_or(line.len());
                let after = line[path_end..].trim_start();
                let assigns = after.starts_with('=') && !after.starts_with("==")
                    || ["+=", "-=", "*=", "/="].iter().any(|op| after.starts_with(op));
                let calls_mut = ["insert(", "push(", "push_str(", "clear(", "remove(", "retain(", "truncate("]
                    .iter()
                    .any(|m| line[..path_end].ends_with(&m[..m.len() - 1]) && after.starts_with('('));
                assert!(!assigns && !calls_mut, "the {field} setting is written: `self.{field}{}`", &line[..line.len().min(60)]);
            }
        }
    }
}

const TOKEN: &str = "usdc.testnet";
const LP: &str = "lp1.testnet";
const BUYER: &str = "ama.testnet";
const USDC: u128 = 1_000_000;
const MIN: u64 = 60_000_000_000;
const T0: u64 = 1_800_000_000_000_000_000;

fn at(who: &str, time: u64, yocto: u128) {
    testing_env!(VMContextBuilder::new()
        .current_account_id(acc(ME))
        .predecessor_account_id(acc(who))
        .attached_deposit(NearToken::from_yoctonear(yocto))
        .block_timestamp(time)
        .build());
}

fn verdict(e: &mut Escrow, id: u64, time: u64, v: Verdict, reason: &str) {
    at("anyone.testnet", time, 0);
    let attempt = e.begin_claim(id);
    at(ME, time, 0);
    let o = e.get_order(id).unwrap();
    let key_fp = e.get_lp(o.lp).unwrap().key_fp.unwrap();
    e.on_verdict(id, attempt, VerifierResult::Output(VerifierOutput { verdict: v, reference: o.reference, key_fp, reason: reason.into() }));
}

#[test]
fn settings_are_identical_after_every_kind_of_flow() {
    at(ME, T0, 0);
    let mut e = init_with(verifier(), params());
    let before = e.get_config();

    // LP setup: register, quote, deposits, secret change, pause/resume.
    at(LP, T0, 100_000_000_000_000_000_000_000);
    let quote = Quote { rate_pesewas_per_usdc: U128(1_500), min_usdc: U128(USDC), max_usdc: U128(50 * USDC), new_buyer_exposure_max: U128(100 * USDC) };
    e.register_lp("paystack".into(), SecretRef { profile: "sec".into(), account_id: acc(LP) }, quote.clone());
    at(TOKEN, T0, 0);
    e.ft_on_transfer(acc(LP), U128(50 * USDC), "bond".into()).detach();
    e.ft_on_transfer(acc(LP), U128(100 * USDC), "liquidity".into()).detach();
    at(LP, T0, 1);
    e.update_quote(quote);
    at(LP, T0, 1);
    e.pause_lp();
    at(LP, T0, 1);
    e.resume_lp();
    e.lps.get_mut(&acc(LP)).unwrap().key_fp = Some("ab".repeat(32));

    // Orders: paid, slashed, expired; then every payout path.
    let open = |e: &mut Escrow, usdc: u128| {
        at(BUYER, T0, 100_000_000_000_000_000_000_000);
        e.open_buy_order(acc(LP), U128(usdc), None).id
    };
    let paid = open(&mut e, 2 * USDC);
    let slashed = open(&mut e, 3 * USDC);
    let expired = open(&mut e, 4 * USDC);
    verdict(&mut e, paid, T0 + MIN, Verdict::Paid, "ok");
    verdict(&mut e, slashed, T0 + MIN, Verdict::Unverifiable, "auth_error");
    verdict(&mut e, slashed, T0 + 4 * MIN, Verdict::Unverifiable, "auth_error");
    at("anyone.testnet", T0 + 13 * MIN, 0);
    e.expire_order(expired);
    at("anyone.testnet", T0 + 13 * MIN, 0);
    let _ = e.withdraw_claimable(acc(BUYER));
    at(ME, T0 + 13 * MIN, 0);
    e.on_payout(acc(BUYER), U128(5 * USDC), Err(PromiseError::Failed));
    at(LP, T0 + 13 * MIN, 1);
    let _ = e.withdraw_liquidity(U128(10 * USDC));
    at(LP, T0 + 13 * MIN, 1);
    let _ = e.withdraw_bond(U128(10 * USDC));

    assert_eq!(e.get_config(), before, "token, OutLayer, verifier, params and deploy tag must never change");
}
