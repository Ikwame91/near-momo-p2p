//! Verdict handling (PLAN M1.5, SPEC §4.3–4.5, §6). The OutLayer call itself arrives in M2.2;
//! here the verifier's result is a typed input to the `on_verdict` callback ("mock verifier"),
//! which is exactly what M2.2 will feed it after parsing the OutLayer response.
//!
//! The verdict path never schedules a transfer (PR #11 review): PAID credits the buyer's claimable
//! balance, and `withdraw_claimable` (anyone, with the caller's gas) sends it. A callback that
//! cannot panic for lack of gas cannot leave an order stuck in VERIFYING.
//!
//! Rules: only a verdict for the order's CURRENT attempt, while VERIFYING, counts (T15); it must
//! carry this order's reference and the LP's registered key_fp, otherwise it is UNVERIFIABLE
//! (inv. 10). USDC is released only on PAID (inv. 1). PENDING / UNVERIFIABLE / a failed run never
//! count as NOT_PAID (inv. 2); they mark the order UNVERIFIED and count a failure for M1.6.

use crate::lp::{FT_TRANSFER_MIN_GAS, WITHDRAW_CALLBACK_GAS};
use crate::{Escrow, EscrowExt, Event, OrderState};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::json;
use near_sdk::{env, near, require, AccountId, GasWeight, NearToken, Promise, PromiseError};

#[near(serializers = [json])]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Paid,
    NotPaid,
    Pending,
    Unverifiable,
}

/// The verifier program's output (SPEC §4.4): `{verdict, reference, key_fp, reason}`.
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub struct VerifierOutput {
    pub verdict: Verdict,
    pub reference: String,
    pub key_fp: String,
    pub reason: String,
}

/// What reached the callback: the program's output, or nothing usable (OutLayer `null`, no value,
/// not JSON, yield timeout). The latter is always `OUTAGE` (D-012).
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub enum VerifierResult {
    Output(VerifierOutput),
    RunFailed,
}

/// Verifier reasons that are the LP's fault (D-012, D-017): key rejected (HTTP 401) or missing.
/// Every other reason, and anything unknown, is an outage.
const LP_FAULT_REASONS: [&str; 2] = ["auth_error", "no_key"];
/// A VERIFYING order with no verdict after this long can be re-claimed by anyone; the missing
/// answer counts as an outage failure and a late callback is ignored (its attempt is stale).
/// Well above OutLayer's yield timeout (~200 blocks).
pub const VERIFY_TIMEOUT_NS: u64 = 15 * 60 * 1_000_000_000;

#[near]
impl Escrow {
    /// Start a verification (SPEC §4.3). Anyone may call it (the buyer, the LP, a keeper):
    /// AWAITING_PAYMENT until `last_claim` (inclusive; `expire_order` is strictly after),
    /// UNVERIFIED at any time (a re-check), or VERIFYING after `VERIFY_TIMEOUT_NS` with no verdict
    /// (counted as an outage failure; if that completes the slash window the order is slashed and
    /// 0 is returned). Returns the new attempt number, which the verdict must carry. M2.2 adds the
    /// OutLayer call and the contract-set fee.
    pub fn claim(&mut self, order_id: u64) -> u32 {
        let now = env::block_timestamp();
        let grace = self.params.t_grace_ns;
        let order = self.orders.get_mut(&order_id).unwrap_or_else(|| env::panic_str("no such order"));
        match order.state {
            OrderState::AwaitingPayment => {
                require!(now <= order.last_claim(grace), "claim window has passed; the order can be expired");
            }
            OrderState::Unverified => {}
            OrderState::Verifying => {
                let since = order.claimed_at.expect("set by claim").0;
                let stuck_after = since.checked_add(VERIFY_TIMEOUT_NS).unwrap_or_else(|| env::panic_str("overflow"));
                require!(now > stuck_after, "a verification is already in flight");
                // Stamp the missing answer when it was DUE, not when someone re-claims (PR #12
                // review): otherwise a caller could pick the time that completes the window.
                record_failure(order, stuck_after, false);
                Event::OrderUnverified {
                    order_id,
                    attempt: order.attempt,
                    failures: order.failures,
                    lp_fault: order.lp_fault,
                }
                .emit();
                // A missing answer only completes the slash window if the stuck check itself started
                // after the window was met; otherwise a fresh check, run now, decides (PR #12 review).
                let (t_retry, t_outage) = (self.params.t_retry_ns, self.params.t_outage_max_ns);
                let started_after_window = due_at(order, t_retry, t_outage).is_some_and(|d| since >= d);
                if started_after_window && self.slash_if_due(order_id) {
                    return 0;
                }
                let order = self.orders.get_mut(&order_id).expect("order exists");
                order.state = OrderState::Verifying;
                order.attempt = order.attempt.checked_add(1).unwrap_or_else(|| env::panic_str("overflow"));
                order.claimed_at = Some(U64(now));
                let attempt = order.attempt;
                Event::OrderVerifying { order_id, attempt }.emit();
                return attempt;
            }
            _ => env::panic_str("order cannot be claimed in its current state"),
        }
        order.state = OrderState::Verifying;
        order.attempt = order.attempt.checked_add(1).unwrap_or_else(|| env::panic_str("overflow"));
        order.claimed_at = Some(U64(now));
        let attempt = order.attempt;
        Event::OrderVerifying { order_id, attempt }.emit();
        attempt
    }

    /// Verdict entry point used by tests and, until M2.2, the only way to deliver a verdict. M2.2
    /// adds the OutLayer callback, which reads the raw promise result, maps anything unparseable to
    /// `RunFailed` (never panics) and calls `apply_verdict`. Returns false if ignored as stale.
    #[private]
    pub fn on_verdict(&mut self, order_id: u64, attempt: u32, result: VerifierResult) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        self.apply_verdict(order_id, attempt, result)
    }
}

impl Escrow {
    /// Apply a verdict. Never schedules a promise and never panics on verifier content, so it
    /// cannot fail for lack of gas inside a callback (PR #11 review).
    pub(crate) fn apply_verdict(&mut self, order_id: u64, attempt: u32, result: VerifierResult) -> bool {
        let now = env::block_timestamp();
        let (grace, delay) = (self.params.t_grace_ns, self.params.release_delay_ns);
        let Some(order) = self.orders.get(&order_id) else { return false };
        if order.state != OrderState::Verifying || order.attempt != attempt {
            return false; // a late or duplicate callback for an older request (T15)
        }
        let (reference, lp_id) = (order.reference.clone(), order.lp.clone());
        // D-022: a check that started after pay_deadline + t_grace can only find a late payment, so
        // PENDING there is NOT_PAID; otherwise a buyer could keep a charge pending and slash an LP
        // without paying (PR #12 review). Before that, PENDING stays an outage (Ama approving late).
        let late_check = order.claimed_at.is_some_and(|c| {
            c.0 > order.pay_deadline.0.checked_add(grace).unwrap_or_else(|| env::panic_str("overflow"))
        });
        let key_fp = self.lps.get(&lp_id).and_then(|l| l.key_fp.clone());

        // Decide the outcome; anything that does not match this order is UNVERIFIABLE (inv. 10).
        let outcome = match result {
            VerifierResult::RunFailed => Outcome::Failure { lp_fault: false },
            VerifierResult::Output(o) => {
                if key_fp.as_deref() != Some(o.key_fp.as_str()) {
                    Outcome::Failure { lp_fault: true } // different key than registered (T24)
                } else if o.reference != reference {
                    Outcome::Failure { lp_fault: false }
                } else {
                    match o.verdict {
                        Verdict::Paid => Outcome::Paid,
                        Verdict::NotPaid => Outcome::NotPaid,
                        Verdict::Pending if late_check => Outcome::NotPaid,
                        Verdict::Pending => Outcome::Failure { lp_fault: false },
                        Verdict::Unverifiable => {
                            Outcome::Failure { lp_fault: LP_FAULT_REASONS.contains(&o.reason.as_str()) }
                        }
                    }
                }
            }
        };

        let order = self.orders.get_mut(&order_id).expect("checked above");
        match outcome {
            Outcome::Paid => {
                if order.new_buyer && delay > 0 {
                    let at = now.checked_add(delay).unwrap_or_else(|| env::panic_str("overflow"));
                    order.state = OrderState::ReleasePending;
                    order.release_at = Some(U64(at));
                    Event::OrderReleasePending { order_id, release_at: U64(at) }.emit();
                } else {
                    self.release(order_id);
                }
            }
            Outcome::NotPaid => {
                // A clear answer ends the unverified streak.
                order.failures = 0;
                order.first_failure_at = None;
                order.last_failure_at = None;
                order.lp_fault = false;
                order.first_lp_fault_at = None;
                // Expire only if the check itself started after the deadline (any payment found
                // then would be late anyway). A check that started earlier may have been answered
                // late: give the buyer a fresh grace period to claim again (PR #11 review).
                let claimed_at = order.claimed_at.map_or(now, |t| t.0);
                if claimed_at <= order.pay_deadline.0 {
                    let until = now.checked_add(grace).unwrap_or_else(|| env::panic_str("overflow"));
                    order.claim_until = Some(U64(until.max(order.last_claim(grace))));
                    order.state = OrderState::AwaitingPayment;
                    Event::OrderNotPaid { order_id, expired: false }.emit();
                } else if now <= order.last_claim(grace) {
                    order.state = OrderState::AwaitingPayment; // still inside the window, no extension
                    Event::OrderNotPaid { order_id, expired: false }.emit();
                } else {
                    order.state = OrderState::Expired;
                    let (amount, new_buyer) = (order.usdc_amount.0, order.new_buyer);
                    self.unlock(&lp_id, amount, true, new_buyer);
                    Event::OrderNotPaid { order_id, expired: true }.emit();
                }
            }
            Outcome::Failure { lp_fault } => {
                record_failure(order, now, lp_fault);
                Event::OrderUnverified { order_id, attempt, failures: order.failures, lp_fault: order.lp_fault }.emit();
                // Slash here, in the same step, as soon as the window is met: otherwise an LP could
                // dodge a due slash by racing to re-claim (PR #11 review).
                self.slash_if_due(order_id);
            }
        }
        true
    }

    /// Slash a buy order if its unverified streak meets the window (SPEC §4.5, D-012): at least
    /// `min_failed_attempts` failures, the first and last at least `t_retry` apart (any LP_FAULT) or
    /// `t_outage_max` apart (outages only). The buyer gets `usdc_amount` from the LP's bond, as a
    /// claimable balance (no promise, D-021); the locked liquidity goes back to the LP. Returns
    /// whether it slashed. Measured on failure times, so only a recorded failure can make it due.
    pub(crate) fn slash_if_due(&mut self, order_id: u64) -> bool {
        let (min_failures, t_retry, t_outage) =
            (self.params.min_failed_attempts, self.params.t_retry_ns, self.params.t_outage_max_ns);
        let order = self.orders.get_mut(&order_id).expect("order exists");
        if !slash_due(order, min_failures, t_retry, t_outage) {
            return false;
        }
        order.state = OrderState::Slashed;
        let (lp, buyer, amount, new_buyer, lp_fault) =
            (order.lp.clone(), order.buyer.clone(), order.usdc_amount.0, order.new_buyer, order.lp_fault);
        self.slash_lp(&lp, amount, new_buyer);
        let cur = self.claimable.get(&buyer).copied().unwrap_or(0);
        self.claimable.insert(buyer.clone(), cur.checked_add(amount).unwrap_or_else(|| env::panic_str("overflow")));
        Event::OrderSlashed { order_id, lp, buyer, amount: U128(amount), lp_fault }.emit();
        true
    }
}

/// When a failure completes the slash window (D-012, PR #12 review): `first_failure + t_outage_max`,
/// or `first_lp_fault + t_retry` if the streak has an LP_FAULT, whichever is earlier. `None` before
/// the first failure.
pub fn due_at(order: &crate::Order, t_retry: u64, t_outage: u64) -> Option<u64> {
    let first = order.first_failure_at?.0;
    let outage_due = first.saturating_add(t_outage);
    let lp_due = order.first_lp_fault_at.map_or(u64::MAX, |f| f.0.saturating_add(t_retry));
    Some(outage_due.min(lp_due))
}

/// The slash rule. Pure, so it is easy to test and to reason about: UNVERIFIED, at least
/// `min_failures` failures, and the last failure at or after `due_at`. The failures need not be
/// spread out; the LP's defence is that anyone (incl. the LP) can re-check at any time and a clear
/// NOT_PAID ends the streak.
pub fn slash_due(order: &crate::Order, min_failures: u8, t_retry: u64, t_outage: u64) -> bool {
    if order.state != OrderState::Unverified || order.failures < min_failures {
        return false;
    }
    let (Some(due), Some(last)) = (due_at(order, t_retry, t_outage), order.last_failure_at) else { return false };
    last.0 >= due
}

/// Count one failed verification (UNVERIFIED): failures++, first time kept, last time updated,
/// LP_FAULT sticky for the streak (D-012).
fn record_failure(order: &mut crate::Order, now: u64, lp_fault: bool) {
    order.state = OrderState::Unverified;
    order.failures = order.failures.saturating_add(1);
    if order.first_failure_at.is_none() {
        order.first_failure_at = Some(U64(now));
    }
    order.last_failure_at = Some(U64(now));
    order.lp_fault |= lp_fault;
    if lp_fault && order.first_lp_fault_at.is_none() {
        order.first_lp_fault_at = Some(U64(now));
    }
}

#[near]
impl Escrow {
    /// Defensive backstop: slash an UNVERIFIED order whose window is met. Today this can never
    /// succeed, because every write that can make a slash due is followed by `slash_if_due` in the
    /// same step; it stays so a future code path cannot leave a due slash undone. Anyone may call it.
    pub fn slash(&mut self, order_id: u64) {
        require!(self.orders.get(&order_id).is_some(), "no such order");
        require!(self.slash_if_due(order_id), "slash conditions not met");
    }

    /// Release a RELEASE_PENDING order once `release_at` has passed. Anyone may call it.
    pub fn finalize_release(&mut self, order_id: u64) {
        let order = self.orders.get(&order_id).unwrap_or_else(|| env::panic_str("no such order"));
        require!(order.state == OrderState::ReleasePending, "order is not pending release");
        let at = order.release_at.expect("set with ReleasePending").0;
        require!(env::block_timestamp() >= at, "release delay has not passed");
        self.release(order_id);
    }

    /// Pay out an account's claimable balance (credited when a payout transfer failed, e.g. the
    /// account was not registered on the token). Anyone may trigger it; it always pays `account`.
    pub fn withdraw_claimable(&mut self, account: AccountId) -> Promise {
        let amount = self.claimable.remove(&account).unwrap_or(0);
        require!(amount > 0, "nothing claimable");
        self.pay_out(account, amount)
    }

    pub fn get_claimable(&self, account: AccountId) -> U128 {
        U128(self.claimable.get(&account).copied().unwrap_or(0))
    }

    /// Callback after a payout transfer: credit the claimable balance if it failed.
    #[private]
    pub fn on_payout(&mut self, account: AccountId, amount: U128, #[callback_result] result: Result<(), PromiseError>) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        if result.is_ok() {
            return true;
        }
        let cur = self.claimable.get(&account).copied().unwrap_or(0);
        self.claimable.insert(account.clone(), cur.checked_add(amount.0).unwrap_or_else(|| env::panic_str("overflow")));
        Event::ClaimableCredited { account, amount }.emit();
        false
    }
}

enum Outcome {
    Paid,
    NotPaid,
    Failure { lp_fault: bool },
}

impl Escrow {
    /// Final release of a buy order: RELEASED, the LP's lock closes (the USDC leaves the LP), the
    /// buyer's history with this LP grows (D-020), and the USDC is credited to the buyer's
    /// claimable balance (no promise here; `withdraw_claimable` sends it, PR #11 review).
    fn release(&mut self, order_id: u64) {
        let order = self.orders.get_mut(&order_id).expect("order exists");
        order.state = OrderState::Released;
        order.release_at = None;
        let (lp, buyer, amount, new_buyer) = (order.lp.clone(), order.buyer.clone(), order.usdc_amount.0, order.new_buyer);
        self.unlock(&lp, amount, false, new_buyer);
        let key = (lp, buyer.clone());
        let n = self.buyer_released.get(&key).copied().unwrap_or(0);
        self.buyer_released.insert(key, n.checked_add(1).unwrap_or_else(|| env::panic_str("overflow")));
        let cur = self.claimable.get(&buyer).copied().unwrap_or(0);
        self.claimable.insert(buyer.clone(), cur.checked_add(amount).unwrap_or_else(|| env::panic_str("overflow")));
        Event::OrderReleased { order_id, buyer, amount: U128(amount) }.emit();
    }

    /// Send `amount` USDC to `account`; on failure `on_payout` credits a claimable balance, so the
    /// books stay exact (inv. 6). Same gas pattern as withdrawals (D-016).
    pub(crate) fn pay_out(&mut self, account: AccountId, amount: u128) -> Promise {
        Promise::new(self.token.clone())
            .function_call_weight(
                "ft_transfer".to_string(),
                json!({ "receiver_id": account, "amount": U128(amount), "memo": "openramp payout" })
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(1),
                FT_TRANSFER_MIN_GAS,
                GasWeight(1),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(WITHDRAW_CALLBACK_GAS)
                    .with_unused_gas_weight(0)
                    .on_payout(account, U128(amount)),
            )
    }
}
