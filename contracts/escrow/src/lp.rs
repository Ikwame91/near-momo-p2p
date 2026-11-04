//! LP registration, deposits and withdrawals (PLAN M1.2, SPEC §3).
//!
//! USDC arrives only through the configured token's `ft_transfer_call` (NEP-141,
//! https://github.com/near/NEPs/blob/master/neps/nep-0141.md): the token calls our
//! `ft_on_transfer` with `msg = "bond"` or `"liquidity"`. Any problem panics, and NEP-141 then
//! refunds the full amount to the sender (fail closed). Withdrawals debit first and re-credit in
//! a callback if the token transfer fails, so the books never show money that left.

use crate::{Escrow, EscrowExt, Event};
use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_sdk::{
    assert_one_yocto, env, near, require, AccountId, Gas, GasWeight, NearToken, Promise, PromiseError,
    PromiseOrValue,
};

/// Minimum gas for the token's `ft_transfer`. It also receives ALL unused prepaid gas (weight 1),
/// so if a token (e.g. native USDC) ever needs more, the LP fixes it by attaching more gas; with
/// no owner and no setter, a fixed cap could block withdrawals forever.
pub(crate) const FT_TRANSFER_MIN_GAS: Gas = Gas::from_tgas(20);
/// Fixed budget for `on_withdraw` (weight 0, so it never competes with `ft_transfer`). Must stay
/// well above the callback's real cost (measure in the M1.8 sandbox).
pub(crate) const WITHDRAW_CALLBACK_GAS: Gas = Gas::from_tgas(15);
/// `key_fp` is written later by the fingerprint callback (M1.5/M2), but its storage is charged at
/// registration: borsh `Some(String)` of 64 hex chars = 1 + 4 + 64 bytes more than `None`.
pub(crate) const KEY_FP_EXTRA_BYTES: u64 = 68;
/// Longest secret profile name. Registration pays for this length whatever the current name, so
/// a later `set_secret_ref` can never grow storage the LP has not paid for.
pub(crate) const MAX_PROFILE_LEN: usize = 64;
/// The only providers (D-017): "paystack" = buy-side LP (collects cedis), "pawapay" = sell-side
/// LP (pays cedis out).
pub const PROVIDERS: [&str; 2] = ["paystack", "pawapay"];
pub(crate) const PROVIDER_BUY: &str = "paystack";

/// Names the OutLayer secret that holds the LP's provider key (never the key itself, inv. 7).
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub struct SecretRef {
    pub profile: String,
    pub account_id: AccountId,
}

/// The LP's quote. `rate_pesewas_per_usdc`: pesewas per 1 USDC (1_000_000 token units).
/// Applies to new orders only (SPEC §3.4).
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub struct Quote {
    pub rate_pesewas_per_usdc: U128,
    pub min_usdc: U128,
    pub max_usdc: U128,
    pub new_buyer_exposure_max: U128,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub struct Lp {
    pub provider: String,
    pub secret_ref: SecretRef,
    pub quote: Quote,
    /// Security deposit, slashed to buyers (SPEC §3).
    pub bond: U128,
    /// USDC available for new buy orders.
    pub liquidity: U128,
    /// USDC locked in open orders (M1.4).
    pub locked: U128,
    pub open_orders: u32,
    /// USDC locked in open orders of NEW buyers; capped by `quote.new_buyer_exposure_max` (inv. 14).
    pub new_buyer_open: U128,
    pub paused: bool,
    /// sha256 of the LP's provider key, hex; set by the TEE fingerprint run. No orders until set
    /// (SPEC §3.1, T24).
    pub key_fp: Option<String>,
}

/// Serialized as "bond" / "liquidity" (same words as the deposit `msg`), in events and callbacks.
#[near(serializers = [json])]
#[serde(rename_all = "snake_case")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pot {
    Bond,
    Liquidity,
}

fn validate_profile(secret_ref: &SecretRef, lp_id: &AccountId) {
    // ASCII letters, digits, '-' and '_' only: keeps free text (e.g. a phone number) out of public
    // state, and one byte per character so the storage reservation stays exact.
    require!(
        !secret_ref.profile.is_empty()
            && secret_ref.profile.len() <= MAX_PROFILE_LEN
            && secret_ref.profile.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "secret profile must be 1-64 chars of A-Z a-z 0-9 - _"
    );
    // An LP may only point at its own OutLayer secret, never another LP's key (T33).
    require!(&secret_ref.account_id == lp_id, "secret_ref must belong to the caller");
}

fn validate_quote(q: &Quote) {
    require!(q.rate_pesewas_per_usdc.0 > 0, "rate must be > 0");
    require!(q.min_usdc.0 > 0, "min must be > 0");
    require!(q.min_usdc.0 <= q.max_usdc.0, "min must be <= max");
}

#[near]
impl Escrow {
    /// Register the caller as an LP. Attach NEAR for the storage this record uses (T18); any
    /// excess is refunded. `provider` is exactly "paystack" (buy side) or "pawapay" (sell side).
    #[payable]
    pub fn register_lp(&mut self, provider: String, secret_ref: SecretRef, quote: Quote) {
        let lp_id = env::predecessor_account_id();
        require!(self.lps.get(&lp_id).is_none(), "already registered");
        require!(PROVIDERS.contains(&provider.as_str()), "provider must be \"paystack\" or \"pawapay\"");
        validate_profile(&secret_ref, &lp_id);
        validate_quote(&quote);
        let profile_room = (MAX_PROFILE_LEN - secret_ref.profile.len()) as u64;
        let before = env::storage_usage();
        let zero = U128(0);
        self.lps.insert(
            lp_id.clone(),
            Lp {
                provider: provider.clone(),
                secret_ref,
                quote,
                bond: zero,
                liquidity: zero,
                locked: zero,
                open_orders: 0,
                new_buyer_open: zero,
                paused: false,
                key_fp: None,
            },
        );
        self.lps.flush();
        let bytes = env::storage_usage() - before + KEY_FP_EXTRA_BYTES + profile_room;
        let cost = env::storage_byte_cost().saturating_mul(bytes as u128);
        let attached = env::attached_deposit();
        require!(attached >= cost, format!("attach at least {} yoctoNEAR for storage", cost.as_yoctonear()));
        let refund = attached.saturating_sub(cost);
        if !refund.is_zero() {
            Promise::new(lp_id.clone()).transfer(refund).detach();
        }
        Event::LpRegistered { lp: lp_id, provider }.emit();
    }

    /// Change the quote; applies to new orders only. Requires exactly 1 yoctoNEAR: a leaked
    /// function-call key (e.g. from the web app) must not be able to reprice the LP's book.
    #[payable]
    pub fn update_quote(&mut self, quote: Quote) {
        assert_one_yocto();
        validate_quote(&quote);
        let id = env::predecessor_account_id();
        match self.lps.get_mut(&id) {
            Some(lp) => lp.quote = quote.clone(),
            None => env::panic_str("not a registered LP"),
        }
        Event::QuoteUpdated {
            lp: id,
            rate_pesewas_per_usdc: quote.rate_pesewas_per_usdc,
            min_usdc: quote.min_usdc,
            max_usdc: quote.max_usdc,
            new_buyer_exposure_max: quote.new_buyer_exposure_max,
        }
        .emit();
    }

    /// Point at a different OutLayer secret (SPEC §3.6): only with zero open orders (inv. 4), and
    /// it clears `key_fp`, so the LP takes no orders until the new key is fingerprinted.
    /// Storage cannot grow: registration paid for the longest profile name.
    #[payable]
    pub fn set_secret_ref(&mut self, secret_ref: SecretRef) {
        assert_one_yocto();
        let id = env::predecessor_account_id();
        validate_profile(&secret_ref, &id);
        let lp = self.lps.get_mut(&id).unwrap_or_else(|| env::panic_str("not a registered LP"));
        require!(lp.open_orders == 0, "secret is locked while orders are open");
        lp.secret_ref = secret_ref;
        lp.key_fp = None;
        Event::SecretRefChanged { lp: id }.emit();
    }

    /// Stop new orders (open orders run to the end), so an LP can always reach zero open orders
    /// (T28). Requires 1 yoctoNEAR (T34).
    #[payable]
    pub fn pause_lp(&mut self) {
        self.set_paused(true);
    }

    #[payable]
    pub fn resume_lp(&mut self) {
        self.set_paused(false);
    }

    /// NEP-141 receiver. Only the configured token may call it (inv. 9, T26).
    pub fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128> {
        require!(env::predecessor_account_id() == self.token, "only the configured token is accepted");
        require!(amount.0 > 0, "amount must be > 0");
        let pot = match msg.as_str() {
            "bond" => Pot::Bond,
            "liquidity" => Pot::Liquidity,
            _ => env::panic_str("msg must be \"bond\" or \"liquidity\""),
        };
        let mut lp = self.lp_or_panic(&sender_id);
        Self::credit(&mut lp, pot, amount.0);
        self.lps.insert(sender_id.clone(), lp);
        Event::Deposit { lp: sender_id, pot, amount }.emit();
        PromiseOrValue::Value(U128(0)) // keep all of it
    }

    /// Withdraw liquidity that is not locked in an order (SPEC §3.5). Requires exactly 1 yoctoNEAR
    /// (proves a full-access key signed it).
    #[payable]
    pub fn withdraw_liquidity(&mut self, amount: U128) -> Promise {
        assert_one_yocto();
        self.withdraw(Pot::Liquidity, amount.0)
    }

    /// Withdraw bond: only with zero open orders (SPEC §3.6, inv. 4; tested with orders in M1.3).
    #[payable]
    pub fn withdraw_bond(&mut self, amount: U128) -> Promise {
        assert_one_yocto();
        let lp = self.lp_or_panic(&env::predecessor_account_id());
        require!(lp.open_orders == 0, "bond is locked while orders are open");
        self.withdraw(Pot::Bond, amount.0)
    }

    /// Callback after the token transfer of a withdrawal: re-credit if it failed. The caller
    /// check is explicit (not only `#[private]`, which unit tests cannot see). The LP record must
    /// never be removed while a withdrawal is in flight, or a failed transfer could not be re-credited.
    #[private]
    pub fn on_withdraw(
        &mut self,
        lp_id: AccountId,
        pot: Pot,
        amount: U128,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> bool {
        require!(env::predecessor_account_id() == env::current_account_id(), "only the contract itself");
        if result.is_ok() {
            return true;
        }
        let mut lp = self.lp_or_panic(&lp_id);
        Self::credit(&mut lp, pot, amount.0);
        self.lps.insert(lp_id.clone(), lp);
        Event::WithdrawRecredited { lp: lp_id, pot, amount }.emit();
        false
    }

    pub fn get_lp(&self, account_id: AccountId) -> Option<Lp> {
        self.lps.get(&account_id).cloned()
    }
}

impl Escrow {
    fn set_paused(&mut self, paused: bool) {
        assert_one_yocto();
        let id = env::predecessor_account_id();
        let lp = self.lps.get_mut(&id).unwrap_or_else(|| env::panic_str("not a registered LP"));
        if lp.paused == paused {
            return; // no change, no event
        }
        lp.paused = paused;
        if paused {
            Event::LpPaused { lp: id }.emit();
        } else {
            Event::LpResumed { lp: id }.emit();
        }
    }

    /// Lock `amount` of a buy-side LP's liquidity for a new buy order. All order limits are checked
    /// here, in the same step as the lock (PR #9 review): LP registered, provider "paystack", not
    /// paused (inv. 14), `key_fp` set (SPEC §3.1), quote min/max, for a new buyer the per-order cap
    /// and the LP's new-buyer exposure (inv. 14, T29), enough liquidity, and the bond rule
    /// `locked + amount <= bond` (inv. 5), so a slash can always be paid in full from the bond.
    pub(crate) fn lock_for_buy(&mut self, lp_id: &AccountId, amount: u128, new_buyer: bool) {
        require!(amount > 0, "amount must be > 0");
        let new_buyer_cap = self.params.new_buyer_cap.0;
        let lp = self.lps.get_mut(lp_id).unwrap_or_else(|| env::panic_str("not a registered LP"));
        require!(lp.provider == PROVIDER_BUY, "LP does not serve buy orders");
        require!(!lp.paused, "LP is paused");
        require!(lp.key_fp.is_some(), "LP key not fingerprinted yet");
        require!(amount >= lp.quote.min_usdc.0, "amount below the LP's minimum");
        require!(amount <= lp.quote.max_usdc.0, "amount above the LP's maximum");
        // Check everything first, then write (PR #10 review: easier to reason about).
        let exposure = if new_buyer {
            require!(amount <= new_buyer_cap, "amount above the new-buyer cap");
            let e = lp.new_buyer_open.0.checked_add(amount).unwrap_or_else(|| env::panic_str("overflow"));
            require!(e <= lp.quote.new_buyer_exposure_max.0, "LP's new-buyer exposure limit reached");
            e
        } else {
            lp.new_buyer_open.0
        };
        require!(lp.liquidity.0 >= amount, "not enough LP liquidity");
        let locked = lp.locked.0.checked_add(amount).unwrap_or_else(|| env::panic_str("overflow"));
        require!(locked <= lp.bond.0, "bond does not cover this order");
        lp.new_buyer_open.0 = exposure;
        lp.liquidity.0 -= amount;
        lp.locked.0 = locked;
        lp.open_orders = lp.open_orders.checked_add(1).unwrap_or_else(|| env::panic_str("overflow"));
    }

    /// Close an order's lock (used by M1.4+): the locked USDC either goes back to the LP's
    /// liquidity (expired, slashed) or leaves the LP (released to the buyer).
    /// Called exactly once per order, with the amount and new-buyer flag stored on that order, and
    /// a slash MUST debit the bond in the same step (PLAN M1.6).
    pub(crate) fn unlock(&mut self, lp_id: &AccountId, amount: u128, back_to_liquidity: bool, new_buyer: bool) {
        require!(amount > 0, "amount must be > 0");
        let lp = self.lps.get_mut(lp_id).unwrap_or_else(|| env::panic_str("not a registered LP"));
        require!(lp.open_orders > 0, "no open orders");
        require!(lp.locked.0 >= amount, "unlock exceeds locked");
        if new_buyer {
            require!(lp.new_buyer_open.0 >= amount, "unlock exceeds new-buyer exposure");
            lp.new_buyer_open.0 -= amount;
        }
        lp.locked.0 -= amount;
        lp.open_orders -= 1;
        if back_to_liquidity {
            lp.liquidity.0 = lp.liquidity.0.checked_add(amount).unwrap_or_else(|| env::panic_str("overflow"));
        }
    }

    /// Slash for one buy order (M1.6, PLAN "(PR #9 review)"): in ONE step the order's lock closes
    /// back to the LP's liquidity and the same amount is taken from the bond, so `locked <= bond`
    /// keeps holding (inv. 5) and the bond, not the liquidity, pays the buyer.
    pub(crate) fn slash_lp(&mut self, lp_id: &AccountId, amount: u128, new_buyer: bool) {
        self.unlock(lp_id, amount, true, new_buyer);
        let lp = self.lps.get_mut(lp_id).unwrap_or_else(|| env::panic_str("not a registered LP"));
        // The bond rule guaranteed bond >= locked >= amount when the order opened, and the bond
        // cannot be withdrawn while orders are open, so this cannot fail; check anyway (fail closed).
        require!(lp.bond.0 >= amount, "bond does not cover the slash");
        lp.bond.0 -= amount;
    }

    fn lp_or_panic(&self, id: &AccountId) -> Lp {
        match self.lps.get(id) {
            Some(lp) => lp.clone(),
            None => env::panic_str("not a registered LP"),
        }
    }

    fn credit(lp: &mut Lp, pot: Pot, amount: u128) {
        let field = match pot {
            Pot::Bond => &mut lp.bond,
            Pot::Liquidity => &mut lp.liquidity,
        };
        field.0 = field.0.checked_add(amount).unwrap_or_else(|| env::panic_str("overflow"));
    }

    fn withdraw(&mut self, pot: Pot, amount: u128) -> Promise {
        require!(amount > 0, "amount must be > 0");
        let lp_id = env::predecessor_account_id();
        let mut lp = self.lp_or_panic(&lp_id);
        let field = match pot {
            Pot::Bond => &mut lp.bond,
            Pot::Liquidity => &mut lp.liquidity,
        };
        require!(field.0 >= amount, "amount exceeds available balance");
        field.0 -= amount;
        self.lps.insert(lp_id.clone(), lp);
        Event::WithdrawRequested { lp: lp_id.clone(), pot, amount: U128(amount) }.emit();
        Promise::new(self.token.clone())
            .function_call_weight(
                "ft_transfer".to_string(),
                json!({ "receiver_id": lp_id, "amount": U128(amount), "memo": "openramp withdraw" })
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
                    .on_withdraw(lp_id, pot, U128(amount)),
            )
    }
}
