// Buyer page (PLAN M4.1, M4.2): open a buy order, get the Paystack link, pay, claim, withdraw.
// Everything shown comes from the escrow's view methods; every action is a wallet-signed call.
import "./buffer-shim.js"; // keep first
import { ESCROW_ID, DEFAULT_LP, EXPLORER_TX } from "./config.js";
import { view, initWallet, showWalletModal, signOut, send } from "./near.js";
import { calls, storageDeposit } from "./calls.js";
import { ghsPesewasFor, parseUsdc, fmtUsdc, fmtGhs, nextStep } from "./format.js";

const escrow = calls(ESCROW_ID);
const ORDERS_SHOWN = 20;

// Read-only mode: `?view=<account>` shows that account's orders without a wallet (nothing can be
// signed). Everything shown is public chain data anyway.
const viewParam = new URLSearchParams(location.search).get("view") || "";
const VIEW_AS = /^[a-z0-9._-]{2,64}$/.test(viewParam) ? viewParam : null;
/** The account whose orders are shown: the connected wallet, else the read-only one. */
const who = () => state.account || VIEW_AS;

const state = {
  account: null,
  config: null, // { token, params, ... }
  lp: null,
  released: 0, // this buyer's released orders with this LP (new-buyer rule)
  orders: [],
  claimable: 0n,
  busy: false,
  loadError: null,
};

// ---------- tiny DOM helper (text only: chain data never goes through innerHTML) ----------
function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === false || v == null) continue;
    if (k === "onclick") el.addEventListener("click", v);
    else if (k === "oninput") el.addEventListener("input", v);
    else if (v === true) el.setAttribute(k, "");
    else el.setAttribute(k, v);
  }
  for (const c of children.flat()) if (c != null && c !== false) el.append(c);
  return el;
}
const $ = (id) => document.getElementById(id);
const nowNs = () => BigInt(Date.now()) * 1_000_000n;

function notice(kind, text, txHashes = []) {
  const links = txHashes.map((t) => h("a", { href: EXPLORER_TX + t, target: "_blank", rel: "noopener" }, "View transaction"));
  $("notice").replaceChildren(h("div", { class: `banner ${kind}` }, text, links.length ? " " : null, links));
}

// ---------- data ----------
async function load() {
  try {
    if (!state.config) state.config = await view(ESCROW_ID, "get_config");
    state.lp = await view(ESCROW_ID, "get_lp", { account_id: DEFAULT_LP });
    if (who()) {
      const buyer = who();
      const [count, claimable, released] = await Promise.all([
        view(ESCROW_ID, "count_orders_by_buyer", { buyer }),
        view(ESCROW_ID, "get_claimable", { account: buyer }),
        view(ESCROW_ID, "get_buyer_released", { lp: DEFAULT_LP, buyer }),
      ]);
      const from = Math.max(0, count - ORDERS_SHOWN);
      const orders = count ? await view(ESCROW_ID, "list_orders_by_buyer", { buyer, from, limit: ORDERS_SHOWN }) : [];
      state.orders = orders.reverse();
      state.claimable = BigInt(claimable);
      state.released = released;
    } else {
      state.orders = [];
      state.claimable = 0n;
      state.released = 0;
    }
    state.loadError = null;
  } catch (e) {
    state.loadError = e.message || String(e);
  }
  render();
}

/** Run a wallet action, show what happened, then reload. Errors are shown, never swallowed. */
async function act(label, calls) {
  if (state.busy) return;
  state.busy = true;
  render();
  notice("info", `${label}: confirm in your wallet…`);
  try {
    const hashes = await send(calls);
    notice("ok", `${label}: sent.`, hashes);
  } catch (e) {
    notice("bad", `${label} failed: ${e?.message || e}`);
  } finally {
    state.busy = false;
    await load();
  }
}

const fee = () => state.config.params.outlayer_call.fee;

async function withdraw() {
  // The token only pays accounts registered with it (NEP-145), so register first if needed.
  const token = state.config.token;
  const txs = [];
  try {
    const registered = await view(token, "storage_balance_of", { account_id: state.account });
    if (!registered) {
      const bounds = await view(token, "storage_balance_bounds");
      txs.push(storageDeposit(token, state.account, bounds.min));
    }
  } catch (e) {
    return notice("bad", `Could not check your token registration: ${e.message || e}`);
  }
  txs.push(escrow.withdrawClaimable(state.account));
  await act("Withdraw", txs);
}

// ---------- rendering ----------
function renderWallet() {
  $("wallet").replaceChildren(
    ...(state.account
      ? [h("span", { class: "acct", title: state.account }, state.account), h("button", { class: "secondary small", onclick: () => signOut() }, "Sign out")]
      : [h("button", { class: "primary small", onclick: showWalletModal }, "Connect wallet")]),
  );
}

function maxBuyable() {
  const { lp, config } = state;
  let max = BigInt(lp.quote.max_usdc);
  const cap = (v) => { if (BigInt(v) < max) max = BigInt(v); };
  cap(lp.liquidity);
  cap(BigInt(lp.bond) - BigInt(lp.locked)); // bond rule: locked ≤ bond
  if (who() && state.released < config.params.new_buyer_orders) cap(config.params.new_buyer_cap);
  return max > 0n ? max : 0n;
}

let amountText = "2";

let buyKey = null;

function renderBuy() {
  const root = $("buy");
  // Rebuild only when its data changed, so the 5 s refresh never steals focus from the amount field.
  const key = JSON.stringify([state.account, state.lp, state.released, state.busy, state.loadError && !state.config, !!state.config]);
  if (key === buyKey) return;
  buyKey = key;
  if (state.loadError && !state.config) {
    return root.replaceChildren(h("div", { class: "banner bad" }, `Could not load the contract: ${state.loadError}`));
  }
  if (!state.config) return root.replaceChildren(h("div", { class: "card muted" }, "Loading…"));
  const lp = state.lp;
  const head = h("h2", { id: "buy-h" }, "Buy USDC with mobile money");
  if (!lp) return root.replaceChildren(head, h("div", { class: "card" }, `No seller is registered as ${DEFAULT_LP} on this contract.`));

  const rate = BigInt(lp.quote.rate_pesewas_per_usdc);
  const min = BigInt(lp.quote.min_usdc);
  const max = maxBuyable();
  const sellerReady = !lp.paused && lp.key_fp && max >= min;

  const payLine = h("div", { class: "big num" });
  const hint = h("div", { class: "muted" });
  const button = h("button", { class: "primary wide" });

  const update = () => {
    const units = parseUsdc(amountText);
    let problem = null;
    if (!sellerReady) problem = lp.paused ? "This seller is paused." : !lp.key_fp ? "This seller hasn't finished setup." : "This seller has no USDC available right now.";
    else if (units == null) problem = "Enter an amount, for example 2 or 2.5.";
    else if (units < min) problem = `The smallest order is ${fmtUsdc(min)} USDC.`;
    else if (units > max) problem = `The most you can buy now is ${fmtUsdc(max)} USDC.`;
    const ghs = units != null && units > 0n ? ghsPesewasFor(units, rate) : null;
    payLine.textContent = ghs != null ? `GHS ${fmtGhs(ghs)}` : "GHS –";
    hint.textContent = problem || "You have 10 minutes to pay after opening the order.";
    if (!state.account) {
      button.textContent = "Connect wallet to buy";
      button.disabled = false;
      button.onclick = showWalletModal;
    } else {
      button.textContent = state.busy ? "Waiting for your wallet…" : "Open order";
      button.disabled = state.busy || !!problem;
      button.onclick = () =>
        act("Open order", [escrow.openOrder(DEFAULT_LP, units, ghs)]);
    }
  };

  const input = h("input", {
    id: "amount", inputmode: "decimal", autocomplete: "off", value: amountText,
    oninput: (e) => { amountText = e.target.value; update(); },
  });
  update();

  root.replaceChildren(
    head,
    h("div", { class: "card" },
      h("div", { class: "row" },
        h("span", { class: "muted" }, `Seller ${DEFAULT_LP}`),
        h("span", { class: "muted num" }, `1 USDC = GHS ${fmtGhs(rate)}`),
      ),
      h("label", { for: "amount" }, "You get"),
      h("div", { class: "field" }, input, h("span", {}, "USDC")),
      h("div", { class: "pay" }, h("div", { class: "muted" }, "You pay by mobile money"), payLine),
      button,
      h("p", { class: "muted", style: "margin:10px 0 0" }, hint),
      h("p", { class: "muted", style: "margin:6px 0 0" },
        `Seller's bond: ${fmtUsdc(lp.bond)} USDC. Available: ${fmtUsdc(max)} USDC.`),
    ),
  );
}

function timeLeft(untilNs) {
  const s = Number((BigInt(untilNs) - nowNs()) / 1_000_000_000n);
  if (s <= 0) return "0:00";
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
/** A countdown that the 1 s timer updates in place (no re-render, so buttons stay put). */
const countdown = (untilNs) => h("span", { class: "num", "data-until": String(untilNs) }, timeLeft(untilNs));

/** Only ever link to Paystack's checkout host, whatever the chain says. */
const safeCheckout = (url) => (typeof url === "string" && url.startsWith("https://checkout.paystack.com/") ? url : null);

function orderCard(o) {
  const grace = BigInt(state.config.params.t_grace_ns);
  const step = nextStep(o, nowNs(), grace);
  const ghs = `GHS ${fmtGhs(o.ghs_amount_pesewas)}`;
  const btn = (cls, text, onclick) => h("button", { class: cls, disabled: state.busy || !state.account, onclick }, text);
  const check = (text, cls = "primary") => btn(cls, text, () => act("Check payment", [escrow.claim(o.id, fee())]));
  let chip, text, actions = [];
  switch (step) {
    case "checkout":
      chip = ["warn", "Step 1 of 3"];
      text = ["Get your payment link. ", countdown(o.pay_deadline), " left to pay."];
      actions = [btn("primary", "Get payment link", () => act("Get payment link", [escrow.requestCheckout(o.id, fee())]))];
      break;
    case "pay": {
      chip = ["warn", "Step 2 of 3"];
      text = [`Pay ${ghs} by mobile money, then come back and check. `, countdown(o.pay_deadline), " left to pay."];
      const url = safeCheckout(o.checkout_url);
      actions = [
        url ? h("a", { class: "btn primary", href: url, target: "_blank", rel: "noopener" }, "Pay with mobile money") : null,
        check("I've paid: check payment", "secondary"),
      ];
      break;
    }
    case "check":
      chip = ["warn", "Time is up"];
      text = "The time to pay has passed. If you paid in time, check your payment now.";
      actions = [check("Check payment")];
      break;
    case "wait":
      chip = ["info", "Checking"];
      text = "Checking your payment with Paystack. This usually takes under a minute.";
      break;
    case "recheck":
      chip = ["warn", "Not confirmed yet"];
      text = `The payment couldn't be checked (${o.failures} ${o.failures === 1 ? "try" : "tries"}). Check again in a few minutes. If it can never be checked, the seller's bond pays you.`;
      actions = [check("Check again")];
      break;
    case "finalize":
      chip = ["info", "Paid"];
      text = "Payment confirmed. The USDC is released after a short safety delay.";
      actions = [btn("primary", "Finish release", () => act("Finish release", [escrow.finalizeRelease(o.id)]))];
      break;
    case "expire":
      chip = ["bad", "Not paid in time"];
      text = "No payment was confirmed in time. Closing the order frees the seller's USDC. You are not charged.";
      actions = [btn("secondary", "Close order", () => act("Close order", [escrow.expireOrder(o.id)]))];
      break;
    default:
      if (o.state === "RELEASED") { chip = ["ok", "Paid"]; text = `Payment confirmed. ${fmtUsdc(o.usdc_amount)} USDC was released to you.`; }
      else if (o.state === "SLASHED") { chip = ["ok", "Paid from bond"]; text = `The seller's side couldn't be verified, so their bond paid you ${fmtUsdc(o.usdc_amount)} USDC.`; }
      else { chip = ["bad", "Expired"]; text = "This order expired without a confirmed payment."; }
  }
  return h("div", { class: "card order" },
    h("div", { class: "row" },
      h("h3", {}, `${fmtUsdc(o.usdc_amount)} USDC for ${ghs}`),
      h("span", { class: `chip ${chip[0]}` }, chip[1]),
    ),
    h("p", {}, text),
    actions.filter(Boolean).length ? h("div", { class: "actions" }, actions) : null,
    h("p", { class: "ref" }, `Order ${o.id} · ${o.reference}`),
  );
}

let ordersKey = null;

function renderOrders() {
  const root = $("orders");
  if (!who() || !state.config) { ordersKey = null; return root.replaceChildren(); }
  // Rebuild only when the data or an order's next step changed: replacing a button while it is
  // being pressed would swallow the click.
  const grace = BigInt(state.config.params.t_grace_ns);
  const key = JSON.stringify([who(), state.account, state.busy, state.orders, state.orders.map((o) => nextStep(o, nowNs(), grace))]);
  if (key === ordersKey) return;
  ordersKey = key;
  root.replaceChildren(
    h("h2", { id: "orders-h" }, state.account ? "Your orders" : `Orders of ${VIEW_AS}`),
    state.account ? "" : h("p", { class: "muted", style: "margin:0 0 10px" }, "Read-only view. Connect this account's wallet to act on its orders."),
    ...(state.orders.length ? state.orders.map(orderCard) : [h("div", { class: "card muted" }, "No orders yet. Open one above.")]),
  );
}

let claimableKey = null;

function renderClaimable() {
  const root = $("claimable");
  const key = [who(), state.account, state.busy, state.claimable].join("|");
  if (key === claimableKey) return;
  claimableKey = key;
  if (!who() || state.claimable === 0n) return root.replaceChildren();
  root.replaceChildren(
    h("div", { class: "banner ok" },
      h("div", { class: "row" },
        h("strong", {}, `${fmtUsdc(state.claimable)} USDC is ready ${state.account ? "for you" : "to withdraw"}`),
        h("button", { class: "primary small", disabled: state.busy || !state.account, onclick: withdraw }, "Withdraw to my wallet"),
      ),
    ),
  );
}

function render() {
  renderWallet();
  renderClaimable();
  renderBuy();
  renderOrders();
  if (state.loadError && state.config) notice("bad", `Could not refresh: ${state.loadError}`);
}

// ---------- start ----------
$("contract-link").replaceChildren(
  h("a", { href: `https://testnet.nearblocks.io/address/${ESCROW_ID}`, target: "_blank", rel: "noopener" }, `Contract: ${ESCROW_ID}`),
);
render();
load();
initWallet((account) => {
  if (account === state.account) return;
  state.account = account;
  load();
}).catch((e) => notice("bad", `Wallet setup failed: ${e.message || e}`));

// Countdowns tick every second; chain data refreshes every 5 s while the tab is visible.
setInterval(() => {
  for (const el of document.querySelectorAll("[data-until]")) el.textContent = timeLeft(el.dataset.until);
  if (state.orders.length) renderOrders(); // only rebuilds when a deadline has just passed
}, 1000);
setInterval(() => { if (!document.hidden && !state.busy) load(); }, 5000);
