// Run: npm test  (node's built-in test runner)
import test from "node:test";
import assert from "node:assert/strict";
import { ghsPesewasFor, fmtUsdc, fmtGhs, parseUsdc, nextStep } from "./format.js";

test("GHS cost matches the contract: ceil(usdc × rate / 1e6)", () => {
  // testnet order: 2 mUSDC at 1500 pesewas/USDC = 3000 pesewas (escrow-d2 order 0)
  assert.equal(ghsPesewasFor(2_000_000n, 1500n), 3000n);
  assert.equal(ghsPesewasFor(1n, 1500n), 1n); // rounds UP, never 0
  assert.equal(ghsPesewasFor(1_000_001n, 1500n), 1501n);
});

test("amount parsing is exact (6 decimals, no floats)", () => {
  assert.equal(parseUsdc("2"), 2_000_000n);
  assert.equal(parseUsdc("0.1"), 100_000n);
  assert.equal(parseUsdc("1.234567"), 1_234_567n);
  assert.equal(parseUsdc("1.2345678"), null); // too many decimals
  assert.equal(parseUsdc("abc"), null);
  assert.equal(parseUsdc(""), null);
  assert.equal(parseUsdc("-1"), null);
});

test("formatting", () => {
  assert.equal(fmtUsdc("2000000"), "2");
  assert.equal(fmtUsdc("2500000"), "2.5");
  assert.equal(fmtGhs("3000"), "30.00");
  assert.equal(fmtGhs("4505"), "45.05");
});

test("next step per order state", () => {
  const now = 1_000n, grace = 10n;
  const o = (x) => ({ state: "AWAITING_PAYMENT", checkout_url: null, pay_deadline: "2000", claim_until: null, ...x });
  assert.equal(nextStep(o({}), now, grace), "checkout");
  assert.equal(nextStep(o({ checkout_url: "https://checkout.paystack.com/x" }), now, grace), "pay");
  // after pay_deadline but inside grace: can still be checked, no new checkout
  assert.equal(nextStep(o({ pay_deadline: "995" }), now, grace), "check");
  // after pay_deadline + grace: only expiry is left
  assert.equal(nextStep(o({ pay_deadline: "900" }), now, grace), "expire");
  // a later claim window (claim_until) keeps the order checkable
  assert.equal(nextStep(o({ pay_deadline: "900", claim_until: "1500" }), now, grace), "check");
  assert.equal(nextStep(o({ state: "UNVERIFIED" }), now, grace), "recheck");
  assert.equal(nextStep(o({ state: "VERIFYING" }), now, grace), "wait");
  assert.equal(nextStep(o({ state: "RELEASED" }), now, grace), "done");
});
