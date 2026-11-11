// Run: npm test  (node's built-in test runner)
import test from "node:test";
import assert from "node:assert/strict";
import {
  ghsPesewasFor, fmtUsdc, fmtGhs, parseUsdc, nextStep,
  flowScreen, statusPill, timeAgo, shortAccount, amountIssue, humanError, payMinutes, clockLeft, ringFraction,
} from "./format.js";

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

test("guided screen and stepper step", () => {
  const now = 1_000n, grace = 10n;
  const o = (x) => ({ state: "AWAITING_PAYMENT", checkout_url: null, pay_deadline: "2000", claim_until: null, ...x });
  assert.deepEqual(flowScreen(null, now, grace), { screen: "amount", step: 1 });
  assert.deepEqual(flowScreen(o({}), now, grace), { screen: "pay", step: 2 });
  assert.deepEqual(flowScreen(o({ checkout_url: "https://checkout.paystack.com/x" }), now, grace), { screen: "pay", step: 2 });
  assert.deepEqual(flowScreen(o({ state: "VERIFYING" }), now, grace), { screen: "verify", step: 3 });
  assert.deepEqual(flowScreen(o({ state: "UNVERIFIED" }), now, grace), { screen: "verify", step: 3 });
  assert.deepEqual(flowScreen(o({ state: "RELEASED" }), now, grace), { screen: "success", step: 4 });
  assert.deepEqual(flowScreen(o({ state: "SLASHED" }), now, grace), { screen: "slashed", step: 4 });
  assert.deepEqual(flowScreen(o({ state: "EXPIRED" }), now, grace), { screen: "expired", step: 2 });
});

test("status pills", () => {
  assert.deepEqual(statusPill("RELEASED"), { kind: "ok", label: "Paid" });
  assert.deepEqual(statusPill("AWAITING_PAYMENT"), { kind: "pending", label: "Pending" });
  assert.deepEqual(statusPill("VERIFYING"), { kind: "pending", label: "Pending" });
  assert.deepEqual(statusPill("EXPIRED"), { kind: "bad", label: "Expired" });
  assert.deepEqual(statusPill("SLASHED"), { kind: "ok", label: "Slashed" });
});

test("time ago and short account", () => {
  const now = 10_000n * 1_000_000_000n;
  assert.equal(timeAgo(now - 10n * 1_000_000_000n, now), "just now");
  assert.equal(timeAgo(now - 120n * 1_000_000_000n, now), "2m ago");
  assert.equal(timeAgo(now - 7200n * 1_000_000_000n, now), "2h ago");
  assert.equal(shortAccount("ikwame.testnet"), "ikwame.testnet");
  assert.equal(shortAccount("a-very-long-buyer-name.ikwame.testnet").includes("…"), true);
});

test("amount is refused before a wallet opens", () => {
  const limits = { min: 1_000_000n, max: 50_000_000n, paused: false, ready: true };
  assert.equal(amountIssue("2", limits), null);
  assert.match(amountIssue("0.5", limits), /smallest/);
  assert.match(amountIssue("80", limits), /most you can buy/);
  assert.match(amountIssue("nope", limits), /Enter an amount/);
  assert.match(amountIssue("2", { ...limits, paused: true }), /paused/);
  assert.match(amountIssue("2", { ...limits, ready: false }), /hasn't finished/);
  assert.match(amountIssue("2", { ...limits, self: true }), /own seller/);
});

test("errors become a sentence, never raw JSON", () => {
  assert.match(humanError(new Error("User rejected the transaction")), /closed the wallet/);
  assert.match(humanError("Insufficient balance for the transaction"), /enough NEAR/);
  assert.match(humanError('Smart contract panicked: "an LP cannot buy from itself"'), /own seller/);
  assert.match(humanError({
    index: 0,
    kind: { FunctionCallError: { ExecutionError: "Smart contract panicked: an LP cannot buy from itself" } },
  }), /own seller/);
  assert.match(humanError("claim window has passed; the order can be expired"), /payment window/);
  const blob = humanError('{"error":"giant","data":' + "x".repeat(400) + "}");
  assert.equal(blob.includes("{"), false);
  assert.match(blob, /didn't go through/);
});

test("pay timer comes from t_pay_ns", () => {
  assert.equal(payMinutes(600_000_000_000n), 10);
  assert.equal(clockLeft(90_000_000_000n, 30_000_000_000n), "1:00");
  assert.equal(clockLeft(1n, 2n), "0:00");
  assert.equal(ringFraction(600n, 600n, 0n), 1);
  assert.equal(ringFraction(300n, 600n, 0n), 0.5);
  assert.equal(ringFraction(0n, 600n, 1n), 0);
});
