// Pure helpers (no network, no DOM) so they can be unit-tested. All money is BigInt: no floats.

const USDC_UNIT = 1_000_000n; // mUSDC has 6 decimals

/** GHS cost in pesewas, exactly as the escrow computes it (SPEC §4.1): ceil(usdc × rate / 1e6). */
export function ghsPesewasFor(usdcUnits, ratePesewasPerUsdc) {
  return (usdcUnits * ratePesewasPerUsdc + USDC_UNIT - 1n) / USDC_UNIT;
}

/** "2.5" → 2500000n; null if it is not a plain positive decimal with at most 6 places. */
export function parseUsdc(text) {
  const m = /^(\d{1,12})(?:\.(\d{1,6}))?$/.exec(String(text).trim());
  if (!m) return null;
  return BigInt(m[1]) * USDC_UNIT + BigInt((m[2] || "").padEnd(6, "0") || "0");
}

export function fmtUsdc(units) {
  const u = BigInt(units);
  const frac = (u % USDC_UNIT).toString().padStart(6, "0").replace(/0+$/, "");
  return (u / USDC_UNIT).toString() + (frac ? "." + frac : "");
}

export function fmtGhs(pesewas) {
  const p = BigInt(pesewas);
  return (p / 100n).toString() + "." + (p % 100n).toString().padStart(2, "0");
}

/**
 * What the buyer can do next with an order. Times are nanoseconds (BigInt), like the contract.
 * Mirrors the escrow: an AWAITING_PAYMENT order can be claimed until
 * max(pay_deadline + t_grace, claim_until); after that only expire_order is left.
 */
export function nextStep(order, nowNs, graceNs) {
  switch (order.state) {
    case "AWAITING_PAYMENT": {
      const deadline = BigInt(order.pay_deadline);
      const base = deadline + graceNs;
      const until = order.claim_until && BigInt(order.claim_until) > base ? BigInt(order.claim_until) : base;
      if (nowNs > until) return "expire";
      if (nowNs > deadline) return "check";
      return order.checkout_url ? "pay" : "checkout";
    }
    case "VERIFYING": return "wait";
    case "UNVERIFIED": return "recheck";
    case "RELEASE_PENDING": return "finalize";
    default: return "done"; // RELEASED, EXPIRED, SLASHED
  }
}

/** Which guided screen an order is on, and the highlighted stepper step (1–4). */
export function flowScreen(order, nowNs, graceNs) {
  if (!order) return { screen: "amount", step: 1 };
  const next = nextStep(order, nowNs, graceNs);
  if (next === "checkout" || next === "pay") return { screen: "pay", step: 2 };
  if (next === "check" || next === "wait" || next === "recheck" || next === "finalize") return { screen: "verify", step: 3 };
  if (next === "expire" || order.state === "EXPIRED") return { screen: "expired", step: 2 };
  if (order.state === "RELEASED") return { screen: "success", step: 4 };
  if (order.state === "SLASHED") return { screen: "slashed", step: 4 };
  return { screen: "failed", step: 3 };
}

/** Pill for the order list. Pending covers every state that is still in progress. */
export function statusPill(state) {
  switch (state) {
    case "RELEASED": return { kind: "ok", label: "Paid" };
    case "SLASHED": return { kind: "ok", label: "Slashed" };
    case "EXPIRED": return { kind: "bad", label: "Expired" };
    default: return { kind: "pending", label: "Pending" };
  }
}

/** "2m ago" from two nanosecond timestamps. */
export function timeAgo(thenNs, nowNs) {
  const sec = Number((BigInt(nowNs) - BigInt(thenNs)) / 1_000_000_000n);
  if (!Number.isFinite(sec) || sec < 45) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  return `${Math.floor(hr / 24)}d ago`;
}

export function shortAccount(id) {
  const s = String(id || "");
  if (s.length <= 18) return s;
  return `${s.slice(0, 8)}…${s.slice(-8)}`;
}

export function initial(id) {
  const s = String(id || "").trim();
  return (s[0] || "?").toUpperCase();
}

/**
 * Why an amount cannot be ordered, or null when it can.
 * Checked before any wallet call. `min` / `max` are USDC units (BigInt).
 */
export function amountIssue(text, { min, max, paused, ready, self }) {
  if (self) return "You can't buy from your own seller account. Sign out and connect a different wallet.";
  if (paused) return "This seller is paused.";
  if (!ready) return "This seller hasn't finished setup.";
  if (max < min) return "This seller has no USDC available right now.";
  const units = parseUsdc(text);
  if (units == null || units === 0n) return "Enter an amount, for example 2 or 2.5.";
  if (units < min) return `The smallest order is ${fmtUsdc(min)} USDC.`;
  if (units > max) return `The most you can buy now is ${fmtUsdc(max)} USDC.`;
  return null;
}

function errorText(raw) {
  const bits = [];
  const seen = new Set();
  const walk = (value, depth) => {
    if (value == null || depth > 6) return;
    if (typeof value === "string" || typeof value === "number") {
      bits.push(String(value));
      return;
    }
    if (typeof value !== "object" || seen.has(value)) return;
    seen.add(value);
    walk(value.message, depth + 1);
    for (const item of Object.values(value)) walk(item, depth + 1);
  };
  walk(raw, 0);
  return bits.join(" ");
}

/** Wallet and contract failures as a sentence a buyer can act on. Never returns raw JSON. */
export function humanError(raw) {
  const text = errorText(raw);
  const m = text.toLowerCase();
  if (/user (rejected|cancelled|canceled)|rejected the transaction|closed the window|user denied/.test(m))
    return "You closed the wallet before confirming.";
  if (/insufficient|not enough balance|not enough near|does not have enough/.test(m))
    return "This account doesn't have enough NEAR for the network fee.";
  if (/cannot buy from itself/.test(m))
    return "You can't buy from your own seller account.";
  if (/claim window has passed|payment window|order has not expired|has expired/.test(m))
    return "This order's payment window has ended.";
  return "That didn't go through. Try again in a moment.";
}

/** Whole minutes in the contract's pay window (`t_pay_ns`), for the note under the timer. */
export function payMinutes(tPayNs) {
  const sec = Number(BigInt(tPayNs) / 1_000_000_000n);
  return Math.max(1, Math.round(sec / 60));
}

export function clockLeft(deadlineNs, nowNs) {
  const s = Number((BigInt(deadlineNs) - BigInt(nowNs)) / 1_000_000_000n);
  if (!Number.isFinite(s) || s <= 0) return "0:00";
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** 1 when the full pay window remains, 0 when the deadline has passed. */
export function ringFraction(deadlineNs, windowNs, nowNs) {
  const left = BigInt(deadlineNs) - BigInt(nowNs);
  const win = BigInt(windowNs);
  if (left <= 0n || win <= 0n) return 0;
  if (left >= win) return 1;
  return Number(left) / Number(win);
}
