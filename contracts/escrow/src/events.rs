//! NEP-297 events (https://github.com/near/NEPs/blob/master/neps/nep-0297.md), emitted as
//! `EVENT_JSON:{"standard":"openramp","version":"1.0.0","event":...,"data":...}` log lines.
//! They let the web app and indexers follow LP activity. NEVER put personal data here: no phone
//! numbers, keys, secret profile names or salts (inv. 7, T19).

use crate::Pot;
use near_sdk::json_types::{U128, U64};
use near_sdk::{near, AccountId};

#[near(event_json(standard = "openramp"))]
pub enum Event {
    #[event_version("1.0.0")]
    LpRegistered { lp: AccountId, provider: String },
    #[event_version("1.0.0")]
    Deposit { lp: AccountId, pot: Pot, amount: U128 },
    #[event_version("1.0.0")]
    WithdrawRequested { lp: AccountId, pot: Pot, amount: U128 },
    /// The token transfer of a withdrawal failed and the amount went back to the LP's balance.
    #[event_version("1.0.0")]
    WithdrawRecredited { lp: AccountId, pot: Pot, amount: U128 },
    #[event_version("1.0.0")]
    QuoteUpdated {
        lp: AccountId,
        rate_pesewas_per_usdc: U128,
        min_usdc: U128,
        max_usdc: U128,
        new_buyer_exposure_max: U128,
    },
    /// The LP pointed at a different OutLayer secret; `key_fp` is cleared until re-fingerprinted.
    #[event_version("1.0.0")]
    SecretRefChanged { lp: AccountId },
    #[event_version("1.0.0")]
    LpPaused { lp: AccountId },
    #[event_version("1.0.0")]
    LpResumed { lp: AccountId },
    /// A buy order was opened. The buyer pays `ghs_amount_pesewas` with `reference`.
    #[event_version("1.0.0")]
    OrderOpened {
        order_id: u64,
        lp: AccountId,
        buyer: AccountId,
        usdc_amount: U128,
        ghs_amount_pesewas: U64,
        reference: String,
        pay_deadline: U64,
    },
    #[event_version("1.0.0")]
    OrderExpired { order_id: u64 },
    #[event_version("1.0.0")]
    OrderVerifying { order_id: u64, attempt: u32 },
    #[event_version("1.0.0")]
    OrderReleasePending { order_id: u64, release_at: U64 },
    #[event_version("1.0.0")]
    OrderReleased { order_id: u64, buyer: AccountId, amount: U128 },
    /// NOT_PAID: back to awaiting payment, or expired if past the deadline + grace.
    #[event_version("1.0.0")]
    OrderNotPaid { order_id: u64, expired: bool },
    #[event_version("1.0.0")]
    OrderUnverified { order_id: u64, attempt: u32, failures: u8, lp_fault: bool },
    /// A payout transfer failed; the amount is now claimable by `account`.
    #[event_version("1.0.0")]
    ClaimableCredited { account: AccountId, amount: U128 },
    /// The LP's bond paid the buyer (claimable); `lp_fault` says which window applied.
    /// The TEE fingerprinted the LP's key; the LP can now take orders.
    #[event_version("1.0.0")]
    LpFingerprinted { lp: AccountId, key_fp: String },
    /// The TEE created the Paystack checkout for this order.
    #[event_version("1.0.0")]
    OrderCheckout { order_id: u64, checkout_url: String },
    #[event_version("1.0.0")]
    OrderSlashed { order_id: u64, lp: AccountId, buyer: AccountId, amount: U128, lp_fault: bool },
}
