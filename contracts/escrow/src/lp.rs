//! LP registration, deposits and withdrawals (PLAN M1.2, SPEC §3).
//!
//! USDC arrives only through the configured token's `ft_transfer_call` (NEP-141,
//! https://github.com/near/NEPs/blob/master/neps/nep-0141.md): the token calls our
//! `ft_on_transfer` with `msg = "bond"` or `"liquidity"`. Any problem panics, and NEP-141 then
//! refunds the full amount to the sender (fail closed). Withdrawals debit first and re-credit in
//! a callback if the token transfer fails, so the books never show money that left.

use crate::{Escrow, EscrowExt};
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
    pub paused: bool,
    /// sha256 of the LP's provider key, hex; set by the TEE fingerprint run. No orders until set
    /// (SPEC §3.1, T24).
    pub key_fp: Option<String>,
}

#[near(serializers = [json])]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pot {
    Bond,
    Liquidity,
}

fn validate_quote(q: &Quote) {
    require!(q.rate_pesewas_per_usdc.0 > 0, "rate must be > 0");
    require!(q.min_usdc.0 > 0, "min must be > 0");
    require!(q.min_usdc.0 <= q.max_usdc.0, "min must be <= max");
}

#[near]
impl Escrow {
    /// Register the caller as an LP. Attach NEAR for the storage this record uses (T18); any
    /// excess is refunded. `provider` is a plug-in name, e.g. "paystack".
    #[payable]
    pub fn register_lp(&mut self, provider: String, secret_ref: SecretRef, quote: Quote) {
        let lp_id = env::predecessor_account_id();
        require!(self.lps.get(&lp_id).is_none(), "already registered");
        require!(!provider.is_empty() && provider.len() <= 32, "provider name must be 1-32 chars");
        require!(!secret_ref.profile.is_empty() && secret_ref.profile.len() <= 64, "secret profile must be 1-64 chars");
        // An LP may only point at its own OutLayer secret, never another LP's key.
        require!(secret_ref.account_id == lp_id, "secret_ref must belong to the registering LP");
        validate_quote(&quote);
        let before = env::storage_usage();
        let zero = U128(0);
        self.lps.insert(
            lp_id.clone(),
            Lp {
                provider,
                secret_ref,
                quote,
                bond: zero,
                liquidity: zero,
                locked: zero,
                open_orders: 0,
                paused: false,
                key_fp: None,
            },
        );
        self.lps.flush();
        let bytes = env::storage_usage() - before + KEY_FP_EXTRA_BYTES;
        let cost = env::storage_byte_cost().saturating_mul(bytes as u128);
        let attached = env::attached_deposit();
        require!(attached >= cost, format!("attach at least {} yoctoNEAR for storage", cost.as_yoctonear()));
        let refund = attached.saturating_sub(cost);
        if !refund.is_zero() {
            Promise::new(lp_id).transfer(refund).detach();
        }
    }

    /// Change the quote; applies to new orders only. Requires exactly 1 yoctoNEAR: a leaked
    /// function-call key (e.g. from the web app) must not be able to reprice the LP's book.
    #[payable]
    pub fn update_quote(&mut self, quote: Quote) {
        assert_one_yocto();
        validate_quote(&quote);
        let id = env::predecessor_account_id();
        match self.lps.get_mut(&id) {
            Some(lp) => lp.quote = quote,
            None => env::panic_str("not a registered LP"),
        }
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
        self.lps.insert(sender_id, lp);
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
        self.lps.insert(lp_id, lp);
        false
    }

    pub fn get_lp(&self, account_id: AccountId) -> Option<Lp> {
        self.lps.get(&account_id).cloned()
    }
}

impl Escrow {
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
