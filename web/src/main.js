// Buyer page. The screen is a guide; every action is still a wallet-signed call from calls.js.
// Chain data is text only. Nothing here changes method names, args, deposits or gas.
import "./buffer-shim.js"; // keep first
import { ESCROW_ID, DEFAULT_LP, EXPLORER_TX } from "./config.js";
import { view, initWallet, showWalletModal, signOut, send } from "./near.js";
import { calls, storageDeposit } from "./calls.js";
import {
  ghsPesewasFor, parseUsdc, fmtUsdc, fmtGhs, nextStep, flowScreen, statusPill, timeAgo,
  shortAccount, initial, amountIssue, humanError, payMinutes, clockLeft, ringFraction, enclaveOutcome,
} from "./format.js";
import { h, $, icon } from "./dom.js";
import { initTheme, currentTheme, toggleTheme } from "./theme.js";
import { toast } from "./toast.js";
import { countTo, confetti } from "./motion.js";

const escrow = calls(ESCROW_ID);
const ORDERS_SHOWN = 20;
const STEPS = ["Choose amount", "Pay with mobile money", "Verify payment", "Receive USDC"];
const EXPLORER_ACCOUNT = `https://testnet.nearblocks.io/address/${ESCROW_ID}`;
const RING = 2 * Math.PI * 15.5;

const viewParam = new URLSearchParams(location.search).get("view") || "";
const VIEW_AS = /^[a-z0-9._-]{2,64}$/.test(viewParam) ? viewParam : null;
const who = () => state.account || VIEW_AS;

const state = {
  account: null,
  config: null,
  lp: null,
  released: 0,
  orders: [],
  claimable: 0n,
  busy: false,
  busyEnclave: false, // the running action waits on OutLayer (checkout link or payment check)
  loadError: null,
  focusId: null,
  compose: false,
  menu: false,
  celebrated: new Set(),
};
let amountText = "2";
let flowKey = null;
let ordersKey = null;
let lastLoadError = null;
let pendingToast = null;

// The latest transaction this browser sent for each order, so an order's explorer link points at
// its own transaction (not whatever was sent last). Kept per contract; works without storage.
const TX_KEY = `openramp.tx.${ESCROW_ID}`;
let txByOrder = {};
try { txByOrder = JSON.parse(localStorage.getItem(TX_KEY) || "{}") || {}; } catch { txByOrder = {}; }
function rememberTx(orderId, hash) {
  if (orderId == null || !hash) return;
  txByOrder[String(orderId)] = hash;
  try { localStorage.setItem(TX_KEY, JSON.stringify(txByOrder)); } catch { /* this visit only */ }
}
const orderTx = (order) => txByOrder[String(order.id)] || null;

const nowNs = () => BigInt(Date.now()) * 1_000_000n;
const fee = () => state.config.params.outlayer_call.fee;
const safeCheckout = (url) => (typeof url === "string" && url.startsWith("https://checkout.paystack.com/") ? url : null);

function maxBuyable() {
  const { lp, config } = state;
  let max = BigInt(lp.quote.max_usdc);
  const cap = (v) => { if (BigInt(v) < max) max = BigInt(v); };
  cap(lp.liquidity);
  cap(BigInt(lp.bond) - BigInt(lp.locked));
  if (who() && state.released < config.params.new_buyer_orders) cap(config.params.new_buyer_cap);
  return max > 0n ? max : 0n;
}

function focused() {
  if (!state.config) return null;
  if (state.compose) return null;
  const grace = BigInt(state.config.params.t_grace_ns);
  if (state.focusId != null) {
    const picked = state.orders.find((o) => String(o.id) === String(state.focusId));
    if (picked) return picked;
  }
  return state.orders.find((o) => {
    const { screen } = flowScreen(o, nowNs(), grace);
    return screen === "pay" || screen === "verify";
  }) || null;
}

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

// Plain-words result of an OutLayer request, shown after the wallet action finishes.
const ENCLAVE_NO_ANSWER = "The verification service didn't answer. Your money is safe. Try again in a few minutes.";
const ENCLAVE_MESSAGES = {
  "checkout-ready": ["ok", "Your Paystack payment link is ready."],
  "no-answer": ["pending", ENCLAVE_NO_ANSWER],
  paid: ["ok", "Payment confirmed."],
  slashed: ["ok", "The seller's side couldn't be verified, so their bond pays you. You're protected."],
  "seller-key": ["pending", "The seller's Paystack key was rejected. Check again later; if it stays this way, the seller's bond pays you."],
  "not-paid": ["pending", "Paystack hasn't confirmed a payment for this order yet. If you just paid, wait a moment and check again."],
  "still-checking": ["pending", "Still checking with the secure enclave. This page updates by itself."],
};

/** Label for the spinner while an action runs. */
const busyLabel = () => (state.busyEnclave ? "Checking with the secure enclave… this can take up to 2 minutes" : "Waiting for your wallet…");

/**
 * Run a wallet action. meta.enclave = "checkout" | "claim" marks a call that waits on OutLayer: the
 * spinner then names the enclave, and the result is judged from the order before and after.
 */
async function act(label, txs, meta = {}) {
  if (state.busy || !state.account) return;
  state.busy = true;
  state.busyEnclave = !!meta.enclave;
  const before = meta.orderId != null ? state.orders.find((o) => o.id === meta.orderId) : null;
  let sentHash = null;
  let ok = false;
  render();
  if (pendingToast) pendingToast();
  pendingToast = toast("pending", meta.enclave
    ? `${label}: approve in your wallet. Then the secure enclave checks with Paystack; this can take up to 2 minutes.`
    : `${label}: confirm in your wallet…`);
  try {
    const hashes = await send(txs);
    if (pendingToast) pendingToast();
    pendingToast = null;
    const hash = hashes[hashes.length - 1]; // the escrow call is always the last one sent
    if (!meta.enclave) toast("ok", `${label} sent.`, hash ? { href: EXPLORER_TX + hash, text: "View transaction" } : null);
    if (meta.orderId != null) state.focusId = meta.orderId;
    state.compose = false;
    rememberTx(meta.orderId, hash);
    sentHash = hash;
    ok = true;
  } catch (e) {
    if (pendingToast) pendingToast();
    pendingToast = null;
    toast("error", humanError(e));
  } finally {
    state.busy = false;
    state.busyEnclave = false;
    await load();
    if (ok && meta.enclave && before) {
      const after = state.orders.find((o) => o.id === meta.orderId);
      const [kind, message] = ENCLAVE_MESSAGES[enclaveOutcome(meta.enclave, before, after)];
      toast(kind, message, sentHash ? { href: EXPLORER_TX + sentHash, text: "View transaction" } : null);
    }
    // Only after a successful open: a cancelled or failed open stays on the amount screen.
    if (ok && meta.focusNewest && state.orders[0]) {
      rememberTx(state.orders[0].id, sentHash);
      state.focusId = state.orders[0].id;
      state.compose = false;
      flowKey = null;
      render();
    }
  }
}

async function withdraw() {
  const token = state.config.token;
  const txs = [];
  try {
    const registered = await view(token, "storage_balance_of", { account_id: state.account });
    if (!registered) {
      const bounds = await view(token, "storage_balance_bounds");
      txs.push(storageDeposit(token, state.account, bounds.min));
    }
  } catch (e) {
    return toast("error", humanError(e));
  }
  txs.push(escrow.withdrawClaimable(state.account));
  await act("Withdraw", txs);
}

function spinnerButton(label) {
  return h("button", { class: "primary", disabled: true }, h("span", { class: "spin", "aria-hidden": "true" }), label);
}

/** `ended`: the order stopped at `step` (e.g. expired), so steps before it are done and none is current. */
function stepper(step, ended = false) {
  return h("ol", { class: `steps${ended ? " ended" : ""}`, "aria-label": "Buy progress" }, STEPS.map((label, i) => {
    const n = i + 1;
    const current = n === step && !ended;
    const cls = n < step ? "done" : current ? "on" : "";
    return h("li", { class: cls, "aria-current": current ? "step" : false, "aria-label": label },
      h("span", { class: "dot" }, n < step ? icon("check") : String(n)),
      h("span", { class: "lbl" }, label),
    );
  }));
}

function claimableBar() {
  if (!who() || state.claimable === 0n) return null;
  return h("div", { class: "badge", style: "margin-bottom:16px" },
    icon("shield"),
    h("span", {}, `${fmtUsdc(state.claimable)} USDC is ready`, state.account ? "" : " to withdraw"),
    state.account
      ? h("button", { class: "primary", style: "width:auto;min-height:44px;margin-left:auto", disabled: state.busy, onclick: withdraw }, "Withdraw")
      : null,
  );
}

function ring(order) {
  const windowNs = state.config.params.t_pay_ns;
  const wrap = h("div", { class: "ring-wrap", "data-deadline": String(order.pay_deadline), "data-window": String(windowNs) });
  const t = document.createElement("template");
  t.innerHTML = `<svg viewBox="0 0 36 36" aria-hidden="true"><circle class="track" cx="18" cy="18" r="15.5"/><circle class="value" data-ring="1" cx="18" cy="18" r="15.5"/></svg>`;
  wrap.append(t.content.firstChild, h("span", { class: "clock num", "data-clock": "1" }, clockLeft(order.pay_deadline, nowNs())));
  return wrap;
}

function renderAmount() {
  const lp = state.lp;
  if (!lp) {
    return h("div", { class: "card rise" }, stepper(1), h("h1", {}, "Buy USDC"), h("p", {}, `No seller is registered as ${DEFAULT_LP}.`));
  }
  const rate = BigInt(lp.quote.rate_pesewas_per_usdc);
  const min = BigInt(lp.quote.min_usdc);
  const max = maxBuyable();
  const self = state.account === DEFAULT_LP;
  const limits = { min, max, paused: lp.paused, ready: !!lp.key_fp, self };
  const payLine = h("div", { class: "big num" });
  const issue = h("div", { class: "issue", id: "amount-error", role: "alert" });
  const button = state.busy ? spinnerButton(busyLabel()) : h("button", { class: "primary" });
  let paintChips = () => {};

  const update = () => {
    paintChips();
    const problem = amountIssue(amountText, limits);
    const units = parseUsdc(amountText);
    const ghs = units != null && units > 0n ? ghsPesewasFor(units, rate) : 0n;
    countTo(payLine, payLine._n ?? 0n, ghs, (n) => (n > 0n ? `GHS ${fmtGhs(n)}` : "GHS –"));
    issue.textContent = problem || "";
    if (state.busy) return;
    if (self) {
      button.textContent = "Switch wallet";
      button.disabled = false;
      button.onclick = () => { signOut().then(showWalletModal).catch((e) => toast("error", humanError(e))); };
      return;
    }
    if (!state.account) {
      button.textContent = "Connect wallet";
      button.disabled = !!problem;
      button.onclick = () => { if (!amountIssue(amountText, limits)) showWalletModal(); };
    } else {
      button.textContent = "Continue";
      button.disabled = !!problem;
      button.onclick = () => {
        const unitsNow = parseUsdc(amountText);
        const problemNow = amountIssue(amountText, limits);
        if (problemNow || unitsNow == null) return;
        act("Open order", [escrow.openOrder(DEFAULT_LP, unitsNow, ghsPesewasFor(unitsNow, rate))], { focusNewest: true });
      };
    }
  };

  const input = h("input", {
    id: "amount", inputmode: "decimal", autocomplete: "off", "aria-describedby": "amount-error",
    value: amountText,
    oninput: (e) => { amountText = e.target.value; update(); },
  });
  const chipValues = [1, 2, 5, 10];
  const chips = chipValues.map((n) => {
    const units = BigInt(n) * 1_000_000n;
    return h("button", {
      class: amountText === String(n) ? "on" : "",
      disabled: units < min || units > max,
      onclick: () => { amountText = String(n); input.value = amountText; update(); },
    }, String(n));
  });
  paintChips = () => chips.forEach((btn, i) => btn.classList.toggle("on", amountText === String(chipValues[i])));
  update();

  return h("div", { class: "card rise" },
    stepper(1),
    claimableBar(),
    h("h1", {}, "Buy USDC"),
    h("div", { class: "seller" },
      h("div", { class: "row" },
        h("span", { class: "name" }, DEFAULT_LP),
        h("span", { class: "num muted" }, `1 USDC = GHS ${fmtGhs(rate)}`),
      ),
      h("div", { class: "badge" }, icon("shield"), `Seller has ${fmtUsdc(lp.bond)} USDC staked as guarantee`),
    ),
    h("label", { class: "field-label", for: "amount" }, "You receive"),
    h("div", { class: "amount" }, input, h("span", { class: "unit" }, "USDC")),
    h("div", { class: "chips", role: "group", "aria-label": "Quick amounts" }, chips),
    h("div", { class: "payline" }, h("div", { class: "muted" }, "You pay by mobile money"), payLine),
    issue,
    button,
    h("p", { class: "muted", style: "margin:12px 0 0" }, `You have ${payMinutes(state.config.params.t_pay_ns)} minutes to pay after opening the order.`),
  );
}

function renderPay(order) {
  const url = safeCheckout(order.checkout_url);
  const minutes = payMinutes(state.config.params.t_pay_ns);
  const primary = state.busy
    ? spinnerButton(busyLabel())
    : url
      ? h("a", { class: "primary", href: url, target: "_blank", rel: "noopener" }, "Open Paystack checkout")
      : h("button", {
          class: "primary",
          disabled: !state.account,
          onclick: () => act("Open Paystack checkout", [escrow.requestCheckout(order.id, fee())], { orderId: order.id, enclave: "checkout" }),
        }, "Open Paystack checkout");
  return h("div", { class: "card rise" },
    stepper(2),
    h("h1", {}, "Pay with mobile money"),
    ring(order),
    h("p", { class: "note" }, `Pay GHS ${fmtGhs(order.ghs_amount_pesewas)} for ${fmtUsdc(order.usdc_amount)} USDC.`),
    h("p", { class: "note muted" }, `Pay with MTN MoMo / Telecel Cash on Paystack's page, then come back. You have ${minutes} minutes from when the order opened.`),
    primary,
    h("button", {
      class: "secondary",
      disabled: state.busy || !state.account,
      onclick: () => act("Check payment", [escrow.claim(order.id, fee())], { orderId: order.id, enclave: "claim" }),
    }, "I've paid, check payment"),
  );
}

function renderVerify(order) {
  const step = nextStep(order, nowNs(), BigInt(state.config.params.t_grace_ns));
  let title = "Verifying with Paystack inside a secure enclave…";
  let body = "This usually takes under a minute. You can leave this page open.";
  let action = null;
  if (step === "recheck") {
    title = "Not confirmed yet";
    body = `The payment couldn't be checked (${order.failures} ${order.failures === 1 ? "try" : "tries"}). Check again in a few minutes. If it can never be checked, the seller's bond pays you.`;
    action = h("button", {
      class: "primary", disabled: state.busy || !state.account,
      onclick: () => act("Check again", [escrow.claim(order.id, fee())], { orderId: order.id, enclave: "claim" }),
    }, state.busy ? busyLabel() : "Check again");
  } else if (step === "check") {
    title = "The pay window has ended";
    body = "If you paid in time, we can still check. If not, you can close the order. You are not charged.";
    action = h("button", {
      class: "primary", disabled: state.busy || !state.account,
      onclick: () => act("Check payment", [escrow.claim(order.id, fee())], { orderId: order.id, enclave: "claim" }),
    }, state.busy ? busyLabel() : "Check payment");
  } else if (step === "finalize") {
    title = "Payment confirmed";
    body = "The USDC unlocks after a short safety delay, then you can take it to your wallet.";
    action = h("button", {
      class: "primary", disabled: state.busy || !state.account,
      onclick: () => act("Finish release", [escrow.finalizeRelease(order.id)], { orderId: order.id }),
    }, state.busy ? busyLabel() : "Finish release");
  }
  const tipId = `tip-${order.id}`;
  return h("div", { class: "card rise enclave" },
    stepper(3),
    h("div", { class: "orb", "aria-hidden": "true" }, icon("shield")),
    h("h1", {}, title),
    h("p", { class: "muted" }, body),
    action,
    h("div", { class: "tip" },
      h("button", { "aria-expanded": "false", "aria-controls": tipId, onclick: (e) => {
        const open = e.currentTarget.getAttribute("aria-expanded") !== "true";
        e.currentTarget.setAttribute("aria-expanded", open ? "true" : "false");
        e.currentTarget.nextElementSibling.hidden = !open;
      } }, "What is the secure enclave?"),
      h("p", { id: tipId, hidden: true }, "A program in a secure enclave calls Paystack with the seller's key. The key never leaves that enclave. The contract releases USDC only when Paystack says this order was paid."),
    ),
  );
}

function outcome(order, kind) {
  const amount = `${fmtUsdc(order.usdc_amount)} USDC`;
  const tx = orderTx(order);
  const explorer = tx
    ? h("a", { href: EXPLORER_TX + tx, target: "_blank", rel: "noopener" }, "View the transaction")
    : h("a", { href: EXPLORER_ACCOUNT, target: "_blank", rel: "noopener" }, "View the contract on the explorer");
  let title, body, extra = null;
  if (kind === "success") {
    title = "USDC received";
    body = `${amount} was released to you.`;
  } else if (kind === "slashed") {
    title = "You're protected";
    body = `The seller was penalised because their side couldn't be verified. Their bond pays you ${amount}.`;
  } else if (kind === "expired") {
    title = "This order expired";
    body = order.state === "EXPIRED"
      ? "No payment was confirmed in time. You weren't charged, and the seller's USDC has been unlocked."
      : "No payment was confirmed in time. You aren't charged. Closing the order unlocks the seller's USDC.";
    if (nextStep(order, nowNs(), BigInt(state.config.params.t_grace_ns)) === "expire") {
      extra = h("button", {
        class: "secondary", disabled: state.busy || !state.account,
        onclick: () => act("Close order", [escrow.expireOrder(order.id)], { orderId: order.id }),
      }, "Close order");
    }
  } else {
    title = "This order didn't complete";
    body = "Nothing was taken from you. If you already paid, check the order again from the list below.";
  }
  const finished = kind === "success" || kind === "slashed";
  const card = h("div", { class: `card rise outcome${finished ? " success" : " ended"}` },
    finished ? stepper(4) : stepper(2, true),
    finished ? null : h("div", { class: "end-icon", "aria-hidden": "true" }, icon("clock")),
    kind === "success" || kind === "slashed" ? h("div", { class: "tick", "aria-hidden": "true" }, icon("check")) : null,
    h("h1", {}, title),
    h("p", { class: "big num" }, kind === "success" || kind === "slashed" ? amount : ""),
    h("p", {}, body),
    kind === "success" && state.claimable > 0n
      ? h("button", { class: "primary", disabled: state.busy || !state.account, onclick: withdraw }, "Withdraw to my wallet")
      : null,
    kind === "slashed" && state.claimable > 0n
      ? h("button", { class: "primary", disabled: state.busy || !state.account, onclick: withdraw }, "Withdraw to my wallet")
      : null,
    extra,
    h("p", {}, explorer),
    h("button", { class: "secondary", onclick: () => { state.compose = true; state.focusId = null; flowKey = null; render(); } }, "Buy more"),
  );
  return card;
}

function renderFlow() {
  const root = $("flow");
  if (state.loadError && !state.config) {
    flowKey = null;
    return root.replaceChildren(h("div", { class: "card" }, h("h1", {}, "Couldn't load OpenRamp"), h("p", {}, "Check your connection and refresh. No money has moved.")));
  }
  if (!state.config || !state.lp && state.lp !== null) {
    return root.replaceChildren(h("div", { class: "card skeleton", "aria-busy": "true", "aria-label": "Loading" },
      h("div", { class: "sk sk-line" }), h("div", { class: "sk sk-amount" }), h("div", { class: "sk sk-btn" })));
  }
  const order = focused();
  const grace = BigInt(state.config.params.t_grace_ns);
  const screen = order ? flowScreen(order, nowNs(), grace) : { screen: "amount", step: 1 };
  const key = JSON.stringify([
    screen, state.account, state.busy, state.claimable.toString(), state.lp, state.released, order && order.id, order && order.state,
    order && order.checkout_url, order && order.failures, state.compose, amountText && screen.screen === "amount" ? "amt" : "",
  ]);
  if (key === flowKey && screen.screen !== "amount") return;
  if (screen.screen === "amount" && key === flowKey) return;
  flowKey = key;
  const readOnly = VIEW_AS && !state.account
    ? h("p", { class: "muted" }, `Read-only view of ${VIEW_AS}. Connect that wallet to act.`)
    : null;
  let body;
  if (!order || screen.screen === "amount") body = renderAmount();
  else if (screen.screen === "pay") body = renderPay(order);
  else if (screen.screen === "verify") body = renderVerify(order);
  else body = outcome(order, screen.screen);
  root.replaceChildren(...[readOnly, body].filter(Boolean));
  paintClocks();
}

function renderOrders() {
  const root = $("orders");
  if (!who() || !state.config) { ordersKey = null; return root.replaceChildren(); }
  const now = nowNs();
  const grace = BigInt(state.config.params.t_grace_ns);
  const pills = new Map(state.orders.map((o) => [o.id, statusPill(o, now, grace)]));
  // The key holds what is shown, including "time ago" and the pill, so the 1 s timer below
  // rebuilds the list only when one of those has actually changed.
  const key = JSON.stringify([who(), state.orders.map((o) => [o.id, o.state, o.usdc_amount, pills.get(o.id).label, timeAgo(o.opened_at, now), orderTx(o)])]);
  if (key === ordersKey) return;
  ordersKey = key;
  const rows = state.orders.length
    ? state.orders.map((o) => {
        const pill = pills.get(o.id);
        return h("div", { class: "order-row" },
          h("button", {
            onclick: () => { state.focusId = o.id; state.compose = false; flowKey = null; render(); },
            "aria-label": `Order ${o.id}, ${pill.label}, ${fmtUsdc(o.usdc_amount)} USDC`,
          },
            h("span", { class: `pill ${pill.kind}${pill.kind === "pending" ? " pulse" : ""}` }, pill.label),
            h("span", { class: "num" }, `${fmtUsdc(o.usdc_amount)} USDC`),
            h("span", { class: "when" }, timeAgo(o.opened_at, now)),
          ),
          h("a", {
            class: "ext", href: orderTx(o) ? EXPLORER_TX + orderTx(o) : EXPLORER_ACCOUNT, target: "_blank", rel: "noopener",
            "aria-label": orderTx(o) ? `Order ${o.id}: view its transaction on the explorer` : `Order ${o.id}: view the contract on the explorer`,
          }, icon("external")),
        );
      })
    : [h("p", { class: "muted" }, "No orders yet.")];
  root.replaceChildren(
    h("h2", { id: "orders-h" }, state.account ? "My orders" : `Orders of ${VIEW_AS}`),
    h("div", { class: "orders" }, rows),
  );
}

function renderHow() {
  const root = $("how");
  if (root.childElementCount) return;
  const item = (name, title, text) => h("article", {}, icon(name), h("strong", {}, title), h("p", {}, text));
  root.replaceChildren(
    h("h2", { id: "how-h" }, "How it works"),
    h("div", { class: "how-grid" },
      item("lock", "USDC is locked", "The seller's USDC sits in a contract nobody can change."),
      item("phone", "You pay MoMo", "Cedis go to the seller on Paystack. Your number stays on their page."),
      item("shield", "Checked in a secure enclave", "Paystack is asked directly. The seller's key never leaves the enclave."),
      item("check", "USDC is released", "A match releases the USDC. If the seller can't be checked, their bond pays you."),
    ),
    h("p", { class: "foot" }, "Each step is a transaction you confirm in your wallet. Keep about 1 testnet NEAR in it for fees; most of it comes back. ",
      h("a", { href: EXPLORER_ACCOUNT, target: "_blank", rel: "noopener" }, ESCROW_ID)),
  );
}

function renderHeader() {
  const theme = currentTheme();
  const accountBtn = state.account
    ? h("button", {
        class: "wallet", "aria-haspopup": "menu", "aria-expanded": state.menu ? "true" : "false",
        onclick: () => { state.menu = !state.menu; renderHeader(); },
      }, h("span", { class: "avatar" }, initial(state.account)), h("span", { class: "acct" }, shortAccount(state.account)))
    : h("button", { class: "wallet", onclick: showWalletModal }, "Connect");
  $("top-actions").replaceChildren(
    h("button", {
      class: "icon-btn", "aria-label": theme === "dark" ? "Switch to light mode" : "Switch to dark mode",
      "aria-pressed": theme === "dark" ? "true" : "false",
      onclick: () => { toggleTheme(); renderHeader(); },
    }, icon(theme === "dark" ? "sun" : "moon")),
    accountBtn,
  );
  $("top-actions").querySelector(".menu")?.remove();
  if (state.menu && state.account) {
    $("top-actions").append(h("div", { class: "menu", role: "menu" },
      h("button", { role: "menuitem", onclick: () => { state.menu = false; signOut(); } }, "Sign out"),
    ));
  }
}

function render() {
  $("mark").replaceChildren(icon("logo"));
  renderHeader();
  renderFlow();
  renderOrders();
  renderHow();
  if (state.loadError && state.config && state.loadError !== lastLoadError) toast("error", "Couldn't refresh. The last numbers are still on screen.");
  lastLoadError = state.loadError;
  queueCelebrate();
}

let celebrateQueued = false;
function queueCelebrate() {
  if (celebrateQueued) return;
  celebrateQueued = true;
  queueMicrotask(() => {
    celebrateQueued = false;
    const order = focused();
    const card = document.querySelector("#flow .success");
    if (!order || !card || !state.config || state.celebrated.has(order.id)) return;
    const screen = flowScreen(order, nowNs(), BigInt(state.config.params.t_grace_ns)).screen;
    if (screen !== "success" && screen !== "slashed") return;
    state.celebrated.add(order.id);
    confetti(card);
  });
}

function paintClocks() {
  const now = nowNs();
  for (const wrap of document.querySelectorAll("[data-deadline]")) {
    const frac = ringFraction(wrap.dataset.deadline, wrap.dataset.window, now);
    const circle = wrap.querySelector("[data-ring]");
    if (circle) {
      circle.style.strokeDasharray = String(RING);
      circle.style.strokeDashoffset = String(RING * (1 - frac));
    }
    const clock = wrap.querySelector("[data-clock]");
    if (clock) clock.textContent = clockLeft(wrap.dataset.deadline, now);
  }
}

initTheme();
render();
load();
initWallet((account) => {
  if (account === state.account) return;
  state.account = account;
  state.menu = false;
  load();
}).catch((e) => toast("error", humanError(e)));

document.addEventListener("click", (e) => {
  if (!state.menu) return;
  if (e.target.closest(".wallet") || e.target.closest(".menu")) return;
  state.menu = false;
  renderHeader();
});

setInterval(() => {
  paintClocks();
  if (state.config) renderOrders(); // only rebuilds when a pill or "time ago" changed
  const order = focused();
  if (!order || !state.config) return;
  const grace = BigInt(state.config.params.t_grace_ns);
  const screen = flowScreen(order, nowNs(), grace).screen;
  if (screen !== flowKeyScreen(flowKey)) { flowKey = null; renderFlow(); }
}, 1000);
setInterval(() => { if (!document.hidden && !state.busy) load(); }, 5000);

function flowKeyScreen(key) {
  if (!key) return "";
  try { return JSON.parse(key)[0].screen; } catch { return ""; }
}
