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
