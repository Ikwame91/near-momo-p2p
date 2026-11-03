//! Buy orders (PLAN M1.4, SPEC §4.1 and §6). Verification, release and slash arrive in M1.5/M1.6.

use crate::{Escrow, EscrowExt, Event};
use near_sdk::json_types::{U128, U64};
use near_sdk::{env, near, require, AccountId, Promise};

/// Max page size of the list views (keeps a view call well inside the gas limit).
pub const MAX_PAGE: u32 = 50;

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
    /// `ceil(usdc_amount × rate / 1e6)`, fixed at open (T10).
    pub ghs_amount_pesewas: u64,
    /// `opr-<deploy_tag>-<id>`: the only reference this order accepts (inv. 3, 12).
    pub reference: String,
    pub state: OrderState,
    /// Block timestamps in ns, as strings in JSON (U64): they exceed JavaScript's 2^53.
    pub opened_at: U64,
    /// Payment must be made by this time (ns).
    pub pay_deadline: U64,
    /// The buyer was new when the order opened (counts against the new-buyer limits).
    pub new_buyer: bool,
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

#[near]
impl Escrow {
    /// Open a buy order with `lp` for `usdc_amount` (6-decimal units). The caller (the buyer)
    /// attaches NEAR for the order's storage (T18); the excess is refunded. Returns the order.
    #[payable]
    pub fn open_buy_order(&mut self, lp: AccountId, usdc_amount: U128) -> Order {
        let buyer = env::predecessor_account_id();
        require!(buyer != lp, "an LP cannot buy from itself");
        let new_buyer = self.buyer_released.get(&buyer).copied().unwrap_or(0) < self.params.new_buyer_orders;
        let rate = match self.lps.get(&lp) {
            Some(l) => l.quote.rate_pesewas_per_usdc.0,
            None => env::panic_str("not a registered LP"),
        };
        let before = env::storage_usage();
        self.lock_for_buy(&lp, usdc_amount.0, new_buyer);
        let ghs = ghs_amount_pesewas(usdc_amount.0, rate);

        let id = self.next_order_id;
        self.next_order_id += 1;
        let now = env::block_timestamp();
        let order = Order {
            id,
            lp: lp.clone(),
            buyer: buyer.clone(),
            usdc_amount,
            ghs_amount_pesewas: ghs,
            reference: format!("opr-{}-{}", self.deploy_tag, id),
            state: OrderState::AwaitingPayment,
            opened_at: U64(now),
            pay_deadline: U64(now.checked_add(self.params.t_pay_ns).unwrap_or_else(|| env::panic_str("overflow"))),
            new_buyer,
        };
        self.orders.insert(id, order.clone());
        self.lp_orders.entry(lp.clone()).or_default().push(id);
        self.buyer_orders.entry(buyer.clone()).or_default().push(id);
        self.flush_order_maps();

        let cost = env::storage_byte_cost().saturating_mul((env::storage_usage() - before) as u128);
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
            ghs_amount_pesewas: ghs,
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

    /// Orders of `lp`, oldest first, `limit` ≤ 50 from position `from`.
    pub fn list_orders_by_lp(&self, lp: AccountId, from: u32, limit: u32) -> Vec<Order> {
        self.page(self.lp_orders.get(&lp), from, limit)
    }

    /// Orders of `buyer`, oldest first, `limit` ≤ 50 from position `from`.
    pub fn list_orders_by_buyer(&self, buyer: AccountId, from: u32, limit: u32) -> Vec<Order> {
        self.page(self.buyer_orders.get(&buyer), from, limit)
    }

    /// RELEASED buy orders of `buyer` (below `new_buyer_orders` the buyer is new).
    pub fn get_buyer_released(&self, buyer: AccountId) -> u32 {
        self.buyer_released.get(&buyer).copied().unwrap_or(0)
    }
}

impl Escrow {
    fn page(&self, ids: Option<&Vec<u64>>, from: u32, limit: u32) -> Vec<Order> {
        let limit = limit.min(MAX_PAGE) as usize;
        ids.map(|v| {
            v.iter()
                .skip(from as usize)
                .take(limit)
                .filter_map(|id| self.orders.get(id).cloned())
                .collect()
        })
        .unwrap_or_default()
    }

    /// Write the order maps now so `open_buy_order` can measure the storage it used.
    fn flush_order_maps(&mut self) {
        self.lps.flush();
        self.orders.flush();
        self.lp_orders.flush();
        self.buyer_orders.flush();
    }
}
