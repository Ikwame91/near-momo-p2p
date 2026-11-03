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
        ghs_amount_pesewas: u64,
        reference: String,
        pay_deadline: U64,
    },
    #[event_version("1.0.0")]
    OrderExpired { order_id: u64 },
}
