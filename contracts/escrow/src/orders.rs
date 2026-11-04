//! Buy orders (PLAN M1.4, SPEC §4.1 and §6). Verification, release and slash arrive in M1.5/M1.6.

use crate::{Escrow, EscrowExt, Event};
use near_sdk::json_types::{U128, U64};
use near_sdk::{env, near, require, AccountId, Promise};

/// Max page size of the list views (keeps a view call well inside the gas limit).
pub const MAX_PAGE: u32 = 50;
/// Longest Paystack checkout URL stored on an order (M2.2b). Paystack URLs are ~40 chars.
pub const MAX_CHECKOUT_URL: usize = 256;
/// NEAR charges storage for each key-value record plus this fixed overhead
/// (`num_extra_bytes_record` in the protocol's storage config).
pub const RECORD_OVERHEAD: u64 = 40;
/// Bytes the order itself can still grow by after open, pre-charged now (PR #10 review): the
/// checkout URL (`None` 1 byte → `Some` 1 + 4 + 256) and three `Option<U64>` (1 → 9 bytes each).
pub const ORDER_GROWTH_BYTES: u64 = (4 + MAX_CHECKOUT_URL as u64) + 3 * 8;

/// Buy-order states (SPEC §6). Borsh stores the variant index: append only, never reorder.
#[near(serializers = [json, borsh])]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrderState {
    AwaitingPayment,
    Verifying,
    Unverified,
    ReleasePending,
    Released,
    Expired,
    Slashed,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub id: u64,
    pub lp: AccountId,
    pub buyer: AccountId,
    pub usdc_amount: U128,
    /// `ceil(usdc_amount × rate / 1e6)`, fixed at open (T10). A JSON string (U64), like all amounts.
    pub ghs_amount_pesewas: U64,
    /// `opr-<deploy_tag>-<id>`: the only reference this order accepts (inv. 3, 12).
    pub reference: String,
    pub state: OrderState,
    /// Block timestamps in ns, as strings in JSON (U64): they exceed JavaScript's 2^53.
    pub opened_at: U64,
    /// Payment must be made by this time (ns).
    pub pay_deadline: U64,
    /// The buyer was new with this LP when the order opened (new-buyer limits apply).
    pub new_buyer: bool,
    // ---- written later (M1.5, M1.6, M2.2b); storage pre-charged at open ----
    /// Paystack checkout link from `request_checkout` (≤ 256 chars).
    pub checkout_url: Option<String>,
    /// Verification request counter; a callback must carry the current value (T15).
    pub attempt: u32,
    /// Failed verifications in the current unverified streak (slash needs `min_failed_attempts`).
    pub failures: u8,
    pub first_failure_at: Option<U64>,
    pub last_failure_at: Option<U64>,
    /// Any failure in the current streak was the LP's fault (window `t_retry`, else `t_outage_max`).
    pub lp_fault: bool,
    /// When a RELEASE_PENDING order may be finalized.
    pub release_at: Option<U64>,
}

/// `ceil(usdc × rate / 1_000_000)`, rounded UP to protect the LP; > 0 and fits in u64 (SPEC §4.1).
pub fn ghs_amount_pesewas(usdc_amount: u128, rate_pesewas_per_usdc: u128) -> u64 {
    let product = usdc_amount
        .checked_mul(rate_pesewas_per_usdc)
        .unwrap_or_else(|| env::panic_str("amount × rate overflows"));
    let pesewas = product.div_ceil(1_000_000);
    require!(pesewas > 0, "GHS amount rounds to zero");
    u64::try_from(pesewas).unwrap_or_else(|_| env::panic_str("GHS amount too large"))
}

/// borsh size of a String key part: 4-byte length + bytes.
fn str_len(s: &str) -> u64 {
    4 + s.len() as u64
}

#[near]
impl Escrow {
    /// Open a buy order with `lp` for `usdc_amount` (6-decimal units). The caller (the buyer)
    /// attaches NEAR for the order's storage, including room for what later steps write (T18);
    /// the excess is refunded. `max_ghs_amount_pesewas`: optional guard against the LP changing its
    /// rate between the buyer signing and the order opening. Returns the order.
    #[payable]
    pub fn open_buy_order(&mut self, lp: AccountId, usdc_amount: U128, max_ghs_amount_pesewas: Option<U64>) -> Order {
        let buyer = env::predecessor_account_id();
        require!(buyer != lp, "an LP cannot buy from itself");
        let history_key = (lp.clone(), buyer.clone());
        let released = self.buyer_released.get(&history_key).copied();
        let new_buyer = released.unwrap_or(0) < self.params.new_buyer_orders;
        let rate = match self.lps.get(&lp) {
            Some(l) => l.quote.rate_pesewas_per_usdc.0,
            None => env::panic_str("not a registered LP"),
        };
        let ghs = ghs_amount_pesewas(usdc_amount.0, rate);
        if let Some(max) = max_ghs_amount_pesewas {
            require!(ghs <= max.0, "GHS amount above the buyer's maximum");
        }
        let before = env::storage_usage();
        self.lock_for_buy(&lp, usdc_amount.0, new_buyer);

        let id = self.next_order_id;
        self.next_order_id += 1;
        let now = env::block_timestamp();
        let order = Order {
            id,
            lp: lp.clone(),
            buyer: buyer.clone(),
            usdc_amount,
            ghs_amount_pesewas: U64(ghs),
            reference: format!("opr-{}-{}", self.deploy_tag, id),
            state: OrderState::AwaitingPayment,
            opened_at: U64(now),
            pay_deadline: U64(now.checked_add(self.params.t_pay_ns).unwrap_or_else(|| env::panic_str("overflow"))),
            new_buyer,
            checkout_url: None,
            attempt: 0,
            failures: 0,
            first_failure_at: None,
            last_failure_at: None,
            lp_fault: false,
            release_at: None,
        };
        self.orders.insert(id, order.clone());
        self.index_push(&lp, &buyer, id);
        self.flush_order_maps();

        // What later steps will write, charged now (PR #10 review): the order's own growth, and
        // the (lp, buyer) history entry that the first release creates (M1.5).
        // + a claimable-balance entry for the buyer, in case the payout transfer fails (M1.5).
        let mut later = ORDER_GROWTH_BYTES + RECORD_OVERHEAD + 1 + str_len(buyer.as_str()) + 16;
        if released.is_none() {
            later += RECORD_OVERHEAD + 1 + str_len(lp.as_str()) + str_len(buyer.as_str()) + 4;
        }
        let bytes = env::storage_usage() - before + later;
        let cost = env::storage_byte_cost().saturating_mul(bytes as u128);
        let attached = env::attached_deposit();
        require!(attached >= cost, format!("attach at least {} yoctoNEAR for storage", cost.as_yoctonear()));
        let refund = attached.saturating_sub(cost);
        if !refund.is_zero() {
            Promise::new(buyer.clone()).transfer(refund).detach();
        }
        Event::OrderOpened {
            order_id: id,
            lp,
            buyer,
            usdc_amount,
            ghs_amount_pesewas: U64(ghs),
            reference: order.reference.clone(),
            pay_deadline: order.pay_deadline,
        }
        .emit();
        order
    }

    /// Expire an unpaid buy order once `pay_deadline + t_grace` has passed (SPEC §6). Anyone may
    /// call it; the LP's locked USDC goes back to its liquidity.
    pub fn expire_order(&mut self, order_id: u64) {
        let grace = self.params.t_grace_ns;
        let order = self.orders.get_mut(&order_id).unwrap_or_else(|| env::panic_str("no such order"));
        require!(order.state == OrderState::AwaitingPayment, "order is not awaiting payment");
        let expires_at = order.pay_deadline.0.checked_add(grace).unwrap_or_else(|| env::panic_str("overflow"));
        require!(env::block_timestamp() > expires_at, "order has not expired yet");
        order.state = OrderState::Expired;
        let (lp, amount, new_buyer) = (order.lp.clone(), order.usdc_amount.0, order.new_buyer);
        self.unlock(&lp, amount, true, new_buyer);
        Event::OrderExpired { order_id }.emit();
    }

    pub fn get_order(&self, order_id: u64) -> Option<Order> {
        self.orders.get(&order_id).cloned()
    }

    /// Orders of `lp` by position (0 = oldest), `limit` ≤ 50. Use `count_orders_by_lp` to page
    /// from the newest end.
    pub fn list_orders_by_lp(&self, lp: AccountId, from: u32, limit: u32) -> Vec<Order> {
        let n = self.count_orders_by_lp(lp.clone());
        self.page(n, from, limit, |i| self.lp_orders.get(&(lp.clone(), i)).copied())
    }

    pub fn list_orders_by_buyer(&self, buyer: AccountId, from: u32, limit: u32) -> Vec<Order> {
        let n = self.count_orders_by_buyer(buyer.clone());
        self.page(n, from, limit, |i| self.buyer_orders.get(&(buyer.clone(), i)).copied())
    }

    pub fn count_orders_by_lp(&self, lp: AccountId) -> u32 {
        self.lp_order_count.get(&lp).copied().unwrap_or(0)
    }

    pub fn count_orders_by_buyer(&self, buyer: AccountId) -> u32 {
        self.buyer_order_count.get(&buyer).copied().unwrap_or(0)
    }

    /// RELEASED buy orders of `buyer` with `lp`. New-buyer status is per LP (D-020): each LP
    /// trusts only its own history, so a buyer cannot be "graduated" through another LP.
    pub fn get_buyer_released(&self, lp: AccountId, buyer: AccountId) -> u32 {
        self.buyer_released.get(&(lp, buyer)).copied().unwrap_or(0)
    }
}

impl Escrow {
    /// O(1) per order, however many orders the account already has (PR #10 review: a growing
    /// Vec rewritten on every open would eventually make an LP impossible to order from).
    fn index_push(&mut self, lp: &AccountId, buyer: &AccountId, id: u64) {
        let n = self.lp_order_count.get(lp).copied().unwrap_or(0);
        self.lp_orders.insert((lp.clone(), n), id);
        self.lp_order_count.insert(lp.clone(), n.checked_add(1).unwrap_or_else(|| env::panic_str("overflow")));
        let m = self.buyer_order_count.get(buyer).copied().unwrap_or(0);
        self.buyer_orders.insert((buyer.clone(), m), id);
        self.buyer_order_count.insert(buyer.clone(), m.checked_add(1).unwrap_or_else(|| env::panic_str("overflow")));
    }

    fn page(&self, count: u32, from: u32, limit: u32, id_at: impl Fn(u32) -> Option<u64>) -> Vec<Order> {
        let end = from.saturating_add(limit.min(MAX_PAGE)).min(count);
        (from..end).filter_map(|i| id_at(i).and_then(|id| self.orders.get(&id).cloned())).collect()
    }

    /// Write the order maps now so `open_buy_order` can measure the storage it used.
    fn flush_order_maps(&mut self) {
        self.lps.flush();
        self.orders.flush();
        self.lp_orders.flush();
        self.lp_order_count.flush();
        self.buyer_orders.flush();
        self.buyer_order_count.flush();
    }
}
