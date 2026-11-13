// Seller (LP) page, read-only (PLAN M4.3): what an LP has in the escrow and the orders it serves.
// Only view calls; nothing here signs a transaction. Chain data goes in as text only.
import { h, icon } from "./dom.js";
import { fmtUsdc, fmtGhs, timeAgo, shortAccount, statusPill, sellerSummary } from "./format.js";

const ORDERS_SHOWN = 20;

/** Read everything the seller page shows. `view(contract, method, args)` is the JSON-RPC reader. */
export async function loadSeller(view, escrowId, lpId) {
  const [lp, count] = await Promise.all([
    view(escrowId, "get_lp", { account_id: lpId }),
    view(escrowId, "count_orders_by_lp", { lp: lpId }),
  ]);
  const from = Math.max(0, count - ORDERS_SHOWN);
  const orders = count ? await view(escrowId, "list_orders_by_lp", { lp: lpId, from, limit: ORDERS_SHOWN }) : [];
  return { lp, count, orders: orders.reverse() };
}

const STATUS = {
  open: ["ok", "Taking orders"],
  paused: ["neutral", "Paused"],
  "no-key": ["pending", "Key not verified yet"],
};

/** The seller page body. `data` from loadSeller; `nowNs` and `graceNs` are BigInt nanoseconds. */
export function sellerScreen({ data, lpId, escrowId, explorerAccount, nowNs, graceNs, buyHref }) {
  const back = h("a", { class: "back-link", href: buyHref }, "← Back to buying USDC");
  if (!data.lp) {
    return h("div", { class: "card rise" },
      h("h1", {}, "Seller not found"),
      h("p", { class: "muted" }, `${lpId} is not registered as a seller on ${escrowId}.`),
      back);
  }
  const { lp, count, orders } = data;
  const s = sellerSummary(lp);
  const [tone, label] = STATUS[s.status];
  const usdc = (v) => `${fmtUsdc(v)} USDC`;

  const card = h("div", { class: "card rise seller-card" },
    h("div", { class: "seller-head" },
      h("span", { class: "avatar big-avatar", "aria-hidden": "true" }, lpId[0].toUpperCase()),
      h("div", { class: "seller-id" },
        h("h1", { title: lpId }, shortAccount(lpId)),
        h("div", { class: "muted small" }, `Seller on ${lp.provider === "paystack" ? "Paystack (mobile money)" : lp.provider} · read-only`)),
      h("span", { class: `pill ${tone}` }, label)),
    h("div", { class: "trust" },
      h("div", {}, h("div", { class: "trust-value num" }, usdc(lp.bond)), h("div", { class: "trust-label" }, "Guarantee. Paid to a buyer if this seller can't be verified.")),
      h("div", {}, h("div", { class: "trust-value num" }, usdc(s.canServe)), h("div", { class: "trust-label" }, "Available to buy right now.")),
    ),
    h("div", { class: "facts" },
      h("div", {}, h("span", { class: "muted" }, "Liquidity"), h("span", { class: "num" }, usdc(lp.liquidity))),
      h("div", {}, h("span", { class: "muted" }, "Locked in orders"), h("span", { class: "num" }, `${usdc(lp.locked)} · ${lp.open_orders} open`)),
      h("div", {}, h("span", { class: "muted" }, "Price"), h("span", { class: "num" }, `GH₵ ${fmtGhs(lp.quote.rate_pesewas_per_usdc)} per USDC`)),
      h("div", {}, h("span", { class: "muted" }, "Order size"), h("span", { class: "num" }, `${fmtUsdc(lp.quote.min_usdc)} – ${fmtUsdc(lp.quote.max_usdc)} USDC`)),
      h("div", {}, h("span", { class: "muted" }, "New-buyer exposure"), h("span", { class: "num" }, `${fmtUsdc(lp.new_buyer_open)} of ${fmtUsdc(lp.quote.new_buyer_exposure_max)} USDC`)),
      h("div", {}, h("span", { class: "muted" }, "Paystack key"),
        s.keyShort
          ? h("span", { class: "key-ok", title: `sha256 of the seller's Paystack key, computed inside the secure enclave: ${lp.key_fp}` }, icon("check"), h("span", { class: "mono" }, s.keyShort))
          : h("span", { class: "key-missing" }, "Not fingerprinted yet: no orders until it is")),
    ),
    h("p", { class: "muted small" },
      "The key fingerprint is computed by the secure enclave from the seller's own Paystack key, which never leaves the enclave. Every payment check must come back with the same fingerprint."),
    h("p", { class: "small" }, h("a", { href: explorerAccount, target: "_blank", rel: "noopener" }, `Contract: ${escrowId}`), " · ", back),
  );

  const rows = orders.length
    ? orders.map((o) => {
        const pill = statusPill(o, nowNs, graceNs);
        return h("div", { class: "order-row static" },
          h("span", { class: `pill ${pill.kind}${pill.kind === "pending" ? " pulse" : ""}` }, pill.label),
          h("span", { class: "num" }, `${fmtUsdc(o.usdc_amount)} USDC`),
          h("span", { class: "muted small buyer", title: o.buyer }, shortAccount(o.buyer)),
          h("span", { class: "when" }, timeAgo(o.opened_at, nowNs)),
        );
      })
    : [h("p", { class: "muted" }, "No orders yet.")];

  const list = h("section", { "aria-labelledby": "lp-orders-h" },
    h("h2", { id: "lp-orders-h" }, `Orders (${count})`),
    count > orders.length ? h("p", { class: "muted small" }, `Showing the newest ${orders.length}.`) : null,
    h("div", { class: "orders" }, rows));

  return h("div", { class: "seller-page" }, card, list);
}
