# Progress log

Newest entry at the top. Template:
```
## YYYY-MM-DD HH:MM UTC — <session / task>
Done:
Failed / blocked:
Next:
Gotchas:
```

## 2026-11-11 22:20 UTC — OutLayer testnet outage; page fixes, seller page, README, deploy prep, demo script
Done:
- Diagnosed the stuck checkout: OutLayer testnet accepted requests but never ran them
  ("Execution promise failed … Refunded", tx 8iehM5r8Caw2KghuZjCvEE2E1GqCbFWDvW3zY7ma2JBK, a
  script call with no page or wallet involved). Our wasm URL serves the right hash. Kwame confirmed
  other projects failed too. Probe (hello wasm, no order touched) every 30 min until 00:00 GMT,
  resumes 07:00.
- Page: enclave waiting text, plain "didn't answer" message, 1 NEAR note (`9b2eb39`); read-only
  seller page `?seller` (`ac619f3`); judge README + wasm hash fix (`0adf503`); Vercel config and
  steps, not deployed (`e2b3a2c`); demo script with fallback (`1429c06`).
```
$ cd web && npm test
# pass 12
# fail 0
$ npm ci && npm run build   (fresh copy from git)
✓ built in 415ms
```
Failed / blocked:
- OutLayer down: no live checkout or check possible. Order 4 (firmstreet.testnet) UNVERIFIED
  with 2 failures (21:55:18, 21:57:26), left alone. Kwame's full buy with firmstreet still open.
Next: when the probe reports UP, Kwame's full buy (M4.2); merge web/buyer-page; Vercel deploy
(ask first); video.
Gotchas:
- A 300 TGas call needs ~0.31 NEAR free in the wallet up front; 0.5 NEAR total was not enough
  after opening an order.
- `outlayer run` from ikwame.testnet fails at 0.17 NEAR; the probe signs as webtest-d2 through a
  separate outlayer config in the scratchpad.

## 2026-11-11 — Review of the Cursor redesign + five fixes (UI only)
Done:
- Reviewed `0fcfee0` (made with Cursor): calls.js / near.js unchanged, no new packages, bundle
  +21 KB, tests and build pass, no console errors in dev or build. Clicked through real orders:
  pay (countdown + Paystack link, d2 order 3), paid, expired, slashed (escrow-d1 orders) all render.
- Fixed: (1) expired is neutral grey, not red, and its tracker shows no current step; (2) header on
  one row; (3) a cancelled open no longer jumps to an old order; (4) explorer links are per order;
  (5) the orders list follows the clock (time ago, Pending → Expired).
```
$ cd web && npm test
# pass 10
# fail 0
$ npx vite build
✓ built in 474ms
```
Failed / blocked: none. Still open: M4.2 happy path with a buyer account that is not the seller.
Next: Kwame's run with a non-seller Meteor account; LP screen (M4.3); hosting (M4.5); README.

## 2026-11-11 06:50 UTC — Buyer page redesign (UI only; M4.2 manual run still open)
Done:
- Redesigned `web/` on `web/buyer-page`: centered card, light/dark, 4-step buyer flow, live
  USDC/GHS amount, pay countdown from `t_pay_ns`, verify state, outcome cards, toasts, orders list.
- `calls.js` and `near.js` unchanged. No new npm packages. Bundle about +20 KB.
- The connected account is refused before the wallet opens when it is the seller
  (`ikwame.testnet` on this deploy).
```
$ cd web && npm test
# pass 10
# fail 0
$ npm run build
✓ built
```
Failed / blocked:
- Kwame's Meteor run as `ikwame.testnet` was rejected by the contract: an LP cannot buy from
  itself. Meteor showed the panic as raw JSON. The happy path (open → pay → check → withdraw)
  still needs a different testnet account.
Next: Kwame connects a buyer account that is not the LP, then M4.3 LP screen, M4.5 hosting, README.
Gotchas:
- Meteor's own error popup cannot be restyled. Blocking self-buy on the page is what keeps that
  raw JSON from appearing.
- Headless Chrome on this machine would not lay out at a real 375 CSS px; fit was checked by
  forcing the page width and measuring the card.

## 2026-11-10 18:00 UTC — Session 5b: web buyer page (M4.1 done, M4.2 built, manual wallet run open)
Done:
- `web/`: plain JS + Vite page (D-024). Shows the LP's live quote, opens orders, gets the Paystack
  link, checks the payment, closes unpaid orders, withdraws (registers the buyer on the token first
  if needed). `?view=<account>` = read-only view. Money maths in `src/format.js` (BigInt, 4 tests).
- Verified in headless Chromium against escrow-d2: page loads live data, light and dark; wallet
  modal opens and the Meteor popup launches with a login request.
- Every call the page makes (`src/calls.js`) sent with a throwaway key (webtest-d2.ikwame.testnet,
  `scripts/check-calls.mjs`): open (How3ksKDNjdXANGypFZ12oXCmfL7MnWxEpjvXjwbzj5C), request_checkout
  → real Paystack URL (4RndW4v4sLRHj69GEc5E3bL4p1vM976NP67jrbWz9z6b), claim → NOT_PAID
  (5ctr5tiedsoyTfqhfR8g5j6UJzwvsUf84fsLxsdEFx9a), expire (CW15DDwjihoSNhN45tjRduEQks6hxnA78AftAMRo35rT);
  order 2: open, test charge, claim → released (9WCoPJJz9E6vhRVhPDfnHmDqiZXwRKpr3R5QzyELpjur),
  storage_deposit + withdraw (8L49odSnxJAcqg56Go5R7wqLB2Tyn1ST1vsqyPHiEJpw), balance 1 mUSDC.
```
$ cd web && npm test
# pass 4
# fail 0
$ npx vite build
✓ built in 461ms
```
Failed / blocked:
- Could not click through a wallet myself: MyNearWallet is in "limited mode" (no new app
  connections since Nov 1), Meteor's site blocks the cloud browser, Intear's picker did not open a
  popup in headless. Stopped after these tries; the wallet step needs Kwame's manual run.
Next: Kwame runs the page with a testnet wallet (Meteor or Intear); then M4.3 LP screen, M4.5 hosting, README.
Gotchas:
- Browser needs `Buffer` and `global` shims for the NEAR wallet libraries (src/buffer-shim.js, vite.config.js).
- Paystack's charge API answers "Duplicate Transaction Reference" once a checkout exists for the
  reference: after request_checkout, payment can only happen on the checkout page.
- A signed call with 300 TGas needs ~0.31 NEAR free in the account up front. ikwame.testnet is down
  to ~0.2 NEAR: top it up from the faucet before the next deploy.
- Screenshots caught a rendering bug (order cards shown as "[object HTMLDivElement]"); fixed.

## 2026-11-10 17:30 UTC — Session 5: PAID through the real Paystack checkout page on d2 (M2.3b)
Done (escrow-d2.ikwame.testnet, verifier ecc0515c…, deploy_tag ad88e13b; d2 deploy tx
87GcoZAa7FGSD1ST9vCcPt47FzmtyD5nMonMCXYWUk89 on Nov 4):
- Kwame stored the real Paystack test key as profile `paystack-d2` (hash ecc0515c…, calls only via
  escrow-d2). First attempt stored the literal placeholder; caught from his screenshot before any
  fingerprint, re-stored. request_fingerprint → key_fp 9d2894e2… (= the real key's fp on d1, not
  the placeholder's 46b1d2d1…), tx BobGuDx1P3ub7EeQbZtwc9n6WH9paZULDmdDVxZhyi1B.
- Order 0, 2 mUSDC = 30 GHS, ref opr-ad88e13b-0 (7XpaFuhLLkdAgmiMHSheYubpG1eFNSMr9KJ22CVVxeRD).
- request_checkout → TEE created the checkout with the LP's key; `order_checkout` stored
  https://checkout.paystack.com/ewsslgmngqmta6b (9tRHeCYNg8mm9DxdSGFHxkXWZM8dxmVbqXQB5cEAHAun).
  The email fix works.
- Kwame paid on that checkout page (test MoMo, MTN). claim → `order_released` 2 mUSDC
  (5dKzaxnqqLjpCJTSfGvaUWybRa8SbT4aZLZyfCWq6aB6); withdraw → buyer-d2 holds 2 mUSDC
  (3TcS1rFmn6ZEgDYVkFKGfJVPd1RRCjygaEmmPadu3t3V).
Failed / blocked: none. (Headless browser cannot pass Paystack's Cloudflare check, so the payment
step is manual.)
Next: merge demo/d2-checkout; web page M4.1–M4.3; README M5.1; video + submit before Nov 14 23:59 UTC.

## 2026-11-04 22:45 UTC — Session 4c: real OutLayer → Paystack check works on testnet (M2.3)
Done (escrow-d1, LP ikwame.testnet, Kwame's real Paystack TEST key in OutLayer profile `paystack-lp`,
hash 6a9ff46b…, access `from:escrow-d1.ikwame.testnet`):
- set_secret_ref → paystack-lp (6xC5vqjMSKywDWm3G3KCQerh3ktJeT9qs9c5cBTzAXSW); request_fingerprint →
  key_fp 9d2894e2… (5fKvk97fFvMTpXZGiZAN9KLQkU2P4AjHMRtcn2Dq4qU1).
- PAID: order 1, 2 mUSDC = 30 GHS (6TJNHuioXVimkwme4nFtNfXAEwDvmwQxr61QQN49RFp5); paid with a Paystack
  test MoMo charge (MTN test number) for reference opr-f51ae4a2-1; claim → `order_released`
  (9jsoF6soxwd7t1zVjfbn1n2WYSMGhYisF75P2GbcudJ); withdraw → buyer 12 mUSDC (F36rJP9BkckAWsSwHN8SAuTYTRXY5UWU3SjwwJhonUJc).
- NOT_PAID: order 2, 3 mUSDC, never paid (5PVBnibr4kS1LKmay8gGm8NDKvr61hWs1A6aVkYGsRq); claim →
  `order_not_paid`, failures 0 (578ZZGyXfp4WzS2fPAQbaXf3aP5HPmy3CnXb16Cvgb6n); after deadline + grace
  expire_order → EXPIRED, LP unlocked, bond untouched (HEubiXioKTP1GXe8y3QpJNRW7B338KyN5G6eS2HUeWN8).
  LP after: bond 40, liquidity 98, locked 0.
Failed / blocked:
- request_checkout on order 1 returned `http_error` (tx 3AseaWvyxMbTD9SRd7rzjpmbhmUQWyBgpBPvWBhK4mjh):
  Paystack rejects the synthetic email `buyer@openramp.example` (400 invalid_email_address). Fixed in
  source (`buyer@example.com`, accepted in a probe; test added). Needs a verifier rebuild → new hash →
  new escrow deploy (verifier fixed at init) → Kwame stores the secret again for the new hash.
Next: bundle any other verifier fixes, rebuild, redeploy as d2, checkout URL demo; web page M4.1.
Gotchas: the program's JSON output is visible as the outlayer.testnet receipt's SuccessValue in
EXPERIMENTAL_tx_status (how the http_error was found).

## 2026-11-04 22:10 UTC — Session 4b: testnet deploy (M2.8) + slash shown on testnet (M2.4)
Done:
- `scripts/deploy-testnet.sh d1`: musdc-d1.ikwame.testnet (mock-ft, tx GBWBmYH4dYvTwoiAf7iYYB628E4qMzPeMy8NLBWfNrE2),
  escrow-d1.ikwame.testnet deploy + `new` in one tx signed by the escrow (AundrHUg9zp82Z6oZS33bKNcvhBVPKh6GyTtfLLutf5m).
  get_config: token musdc-d1, outlayer outlayer.testnet, deploy_tag f51ae4a2, verifier 6a9ff46b….
- `scripts/lp-setup.sh d1 ikwame.testnet smoke-fake`: mint, register_lp, bond 50, liquidity 100,
  `request_fingerprint` through the escrow → real OutLayer run → callback stored key_fp `931d1516…`
  (tx HyCobpCWuxdmroU88Wq66rq1Bh1kEKBR3WL2W9ftQ3z).
- Slash on testnet with the FAKE key (`scripts/buy.sh`): order 0 (10 mUSDC, opr-f51ae4a2-0,
  tx 42YbW7J5Qa7poTMfNkUqYyNrrzvgsh42ctCCUD4oM15p); claim 1 → UNVERIFIED, lp_fault, failures 1
  (AVz7QqGD7w9ZB1EnZqQeF8ywLtSY4aJiJPwWCxy5XZ7M); claim 2 after t_retry (3 min) → `order_slashed`
  10 mUSDC (Hz3QLHuYosA7oirTpKmHyZ2Ktjvn4QnQbCJ6ggkquDyB); withdraw_claimable → buyer holds 10 mUSDC
  (6DeqoxB6TR8zJBPqSDe5GnZZZb9HEW1wJwQmrjt8Vic4). LP after: bond 40, liquidity 100, locked 0.
Failed / blocked:
- PAID path waits on [K]: real Paystack test key as OutLayer secret profile `paystack-lp` for hash
  6a9ff46b… with `--access allow-all --via escrow-d1.ikwame.testnet` (old `paystack` profile is bound
  to the Test A hash).
Next: set_secret_ref → paystack-lp, request_fingerprint, open → checkout → pay (test MoMo) → claim → PAID.
Gotchas: near CLI output ends with a "console command" line containing `{}`; parse only after
"return value".

## 2026-11-04 — Session 4: OutLayer wiring done (M2.1, M2.1b, M2.2, M2.2b, M2.2c), unit level
- `verifier/` crate (v2): modes fingerprint / checkout / verify; key_fp in every verify output;
  PENDING; only 401 = auth_error; amount, currency and mobile_money channel checked before PAID
  and PENDING; exact `opr-<8hex>-<id>`; checkout via Paystack initialize (GHS, mobile_money,
  synthetic email). 19 tests. wasm sha256 6a9ff46b643c65ef7339c5d6b7f573b55c09f7764c614c101c958d76a440926f.
- TEE smoke test (CLI, FAKE key bound to the hash): fingerprint → `931d1516…` (= sha256 of the fake
  key, tx CEKQRPBEzDUtCrzV9eyJCSNue4cKU7vvh7vX3kegxdaE); verify → UNVERIFIABLE auth_error
  (tx A7euG5BMCNYo6qBurg3M35w9iiu3w9CUctCKA41y5BDT); checkout → error auth_error, i.e. the POST
  reached Paystack (tx FbR7vhtwFx923vsaPuRfBf38138wH6pvnxnL5wsMNB1a). First fingerprint try hit an
  RPC timeout; the retry worked.
- Escrow: payable `claim` (fee, exact request_execution args, callback args built in the contract,
  fee refunded when it slashes), `on_verdict_result` (raw bytes, never panics), `request_fingerprint`
  + `on_fingerprint_result` (64 hex, key_fp unset, no open orders, `secret_gen` unchanged),
  `request_checkout` + `on_checkout_result` (Paystack URL ≤ 256 chars, this order's reference).
  Internal logic renamed `begin_claim` (tests use it). 14 new tests.
```
$ cd contracts && cargo test
test result: ok. 264 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ cd verifier && cargo test
test result: ok. 19 passed; 0 failed
```
Next: testnet deploy (M2.8 script), LP setup, created → PAID, slash.

## 2026-11-04 — Session 4: M1.7 DONE (books check, one test)
`books_tests::books_balance_covers_everything_owed_across_all_flows`: models the contract's USDC
balance (in via ft_on_transfer, out via successful transfers) through deposits, stray tokens, 4
orders (release, failed then successful payout, slash, NOT_PAID then release, expiry), LP
withdrawals; checks balance ≥ owed (inv. 6) and locked ≤ bond (inv. 5) after every step, and at the
end that only the stray tokens are unowed. Sanity: with the slash's bond debit removed it fails
("inv. 6 broken after slash o1 ... balance 297000000 < owed 310000000").
```
$ cargo test -q
test result: ok. 250 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.63s
```
Next: OutLayer wiring (verifier v2 modes, escrow claim → OutLayer, fingerprint, checkout).

## 2026-11-04 — Session 4: PR #12 review fix-ups (D-022)
Reviewers: no fund-safety bug in the slash code, but a buyer-side attack (H1) and two fairness gaps.
Fixed:
- PENDING from a check claimed after pay_deadline + t_grace → NOT_PAID (D-022, Kwame to confirm).
- Stuck-timeout failure stamped at claimed_at + VERIFY_TIMEOUT; only completes the window if the
  stuck check started after it (`due_at`).
- `first_lp_fault_at` (storage pre-charged): t_retry counts from the first LP_FAULT.
- 12 edge tests from the code review adopted after reading them (bond == amount, experienced
  buyer, paused LP, saturating failures, claim_until extension, timeout clocks, existing
  claimable, double public slash, outage event flag, NOT_PAID resets lp_fault); one adapted to
  the new stuck-timeout rule. Event sequences on both timeout paths asserted.
- 3 older tests rewritten because their rule deliberately changed (stuck timeout, LP-fault window,
  pending after grace); none weakened.
- Mutation check on the new logic: 13 mutations, all caught. Clippy nit fixed.
- Process: subagent reviews paused at Kwame's request (D-019 update).
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 249 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.66s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --all-targets 2>&1 | grep -cE '^(warning|error)'
0
```
Next: merge PR #12; M1.7 books invariant.

## 2026-11-04 — Session 4: M1.6 DONE (slash)
Branch: `escrow/slash` (from main after PR #11).
Done:
- `slash_due(order, min_failures, t_retry, t_outage)` (pure): UNVERIFIED, ≥ min_failed_attempts,
  last − first failure ≥ t_retry (any LP_FAULT) or t_outage_max (outages only).
- `slash_if_due` runs in the same step as every recorded failure (verdict callback and the stuck
  timeout re-claim, which returns 0 when it slashes): an LP cannot race a due slash (PR #11 review).
  Public `slash(order_id)` backstop.
- `lp.slash_lp`: unlock back to liquidity AND bond −= amount in one step (PR #9 review); buyer's
  claimable += amount; event `order_slashed`. A slash does not count as a released order.
- One older test (T35, pending after the deadline) moved its first failure so both stay inside
  the 6-min demo outage window: at 12 min apart the slash now correctly fires (D-017 hard limit).
- 18 slash tests (both windows at the exact boundary and 1 ns short, mixed classes, streak reset,
  min attempts, accounting, bond rule with another order open, terminal state, late PAID ignored,
  backstop, timeout path, pure rule). 13 mutations on the slash code: all caught.
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 235 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.50s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --all-targets   (no output)
```
Next: reviewer subagents on the PR, fix, merge; then M1.7 books invariant.

## 2026-11-04 — Session 4: PR #11 review fix-ups (D-021)
Both reviewers flagged the same High: the PAID callback scheduled a 35 TGas payout and failed with
the allowed callback gas, leaving the order stuck in VERIFYING. Fixed:
- `release` credits the buyer's claimable balance (no promise); `withdraw_claimable` pays.
  Test: PAID verdict succeeds with prepaid gas = MIN_CALLBACK_GAS_TGAS (10).
- Verdict logic moved to internal `apply_verdict` (M2.2's raw-bytes callback will call it).
- `claimed_at` + `claim_until` on the order (storage pre-charged): slow NOT_PAID for an early check
  gives a fresh grace period; stuck VERIFYING re-claimable after VERIFY_TIMEOUT (15 min), counted
  as an outage.
- Tests: payout receipts (deposit, gas, weights, callback args), claimable sums, re-credit after a
  failed withdraw, claim on RELEASE_PENDING / RELEASED / EXPIRED fails, exact event payloads,
  new_buyer_open after NOT_PAID expiry, full streak reset, PENDING after deadline (T35), T16 asserts,
  fresh grace, no extension for late checks, stuck timeout. 4 old tests changed because the
  behaviour deliberately changed (PAID credits claimable instead of transferring; clearer
  "in flight" message); none weakened.
- Mutation re-check: 19 mutations incl. all reviewer survivors still applicable: all caught.
- Docs: SPEC §4.1 events, §4.5; THREAT T6, T37–T39; PLAN notes M1.5, M1.6, M2.1, M2.2, M2.7;
  DECISIONS D-021.
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 217 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.87s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --all-targets   (no output)
```
Next: merge PR #11; M1.6 slash.

## 2026-11-04 — Session 4: M1.5 DONE (verdict handling, release, claimable payouts)
Branch: `escrow/verdicts-and-release` (from main after PR #10).
Done (`src/verdicts.rs`, `src/verdicts_tests.rs`):
- `claim(order_id)`: anyone; AWAITING_PAYMENT until pay_deadline + t_grace inclusive (expire is
  strictly after), or UNVERIFIED any time; increments `attempt`; state VERIFYING.
- `on_verdict(order_id, attempt, result)` (#[private] + body check): stale attempt / not
  VERIFYING / unknown order → ignored (returns false). key_fp mismatch → UNVERIFIED, LP_FAULT;
  reference mismatch → UNVERIFIED, outage. PAID → release (or RELEASE_PENDING for a new buyer when
  release_delay > 0). NOT_PAID → AWAITING_PAYMENT before the deadline + grace (streak reset), else
  EXPIRED + unlock. PENDING / UNVERIFIABLE / RunFailed → UNVERIFIED, failures++, first/last
  failure times, lp_fault only for auth_error / no_key / key mismatch (sticky in the streak).
- `finalize_release` (anyone, after release_at). Release: unlock (USDC leaves the LP),
  buyer_released[(lp, buyer)] += 1, ft_transfer to the buyer; `on_payout` credits a claimable
  balance on failure; `withdraw_claimable(account)` (anyone, always pays the owner). Claimable
  entry storage pre-charged at open.
- Events: order_verifying, order_release_pending, order_released, order_not_paid,
  order_unverified, claimable_credited.
- Mutation check on verdicts.rs: 20 mutations (each check, each failure class, boundaries,
  delay rule, streak reset, unlock direction, history, claimable, payout): all caught after adding
  the exact-boundary NOT_PAID test.
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 199 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.93s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --all-targets 2>&1 | grep -cE '^(warning|error)'
0
```
Failed / blocked: test-setup issues only (borrow order in test helpers; mocked storage persists
within a test, so loop tests reuse one escrow).
Next: reviewer subagents on the PR, fix, merge; then M1.6 slash.

## 2026-11-04 — Session 4: PR #10 review fix-ups (D-020)
Reviewers (security + code with 58 mutations) found no way to take USDC. Fixed:
- Order indexes O(1) per order (LookupMap by position + counts) + count views; test proves the
  bytes written per order stay constant (apart from the reference's id digits).
- New-buyer status per (lp, buyer); `get_buyer_released(lp, buyer)`.
- Order fields for later steps added now (checkout_url, attempt, failures, first/last failure,
  lp_fault, release_at); storage for their growth + the first history entry pre-charged.
- `ghs_amount_pesewas` → U64 string; optional `max_ghs_amount_pesewas` guard.
- `lock_for_buy` checks everything before writing.
- Tests: exact storage charge (with forced flush, so a missing contract flush fails), exact deposit
  without refund, 1 yocto short fails, exposure and quote max at exactly the limit, real rate
  change after open (T10), buyer max, per-LP history, mixed expiry, public liquidity check,
  unlock guard, u64::MAX GHS, newest-end paging, empty later fields.
- Mutation re-check: 14 reviewer-style mutations (incl. each map flush, cost/refund maths, both
  `<=` boundaries, the new reservations and the per-LP key): all caught.
- Docs: SPEC §4.1 / inv. 3 / inv. 14; THREAT T7, T10, T18 (no refund), T29; DECISIONS D-020;
  PLAN notes for M1.5, M2.1, M5.1 and new M1.10 [nice].
Test-setup lessons: mocked LookupMap edits made directly in tests must be flushed in the helper,
or the next measured call writes (and is charged for) them.
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 167 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.26s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --all-targets   (no output)
```
Next: merge PR #10; M1.5 verdict handling.

## 2026-11-03 — Session 3: M1.4 DONE (buy orders: open, expire, limits, views)
Branch: `escrow/buy-orders` (from main after PR #9).
Done (`src/orders.rs`, `src/orders_tests.rs`, `lp.rs`, `lib.rs`, `events.rs`):
- `open_buy_order(lp, usdc_amount)` (payable: buyer pays order storage, excess refunded):
  reference `opr-<deploy_tag>-<id>`, `ghs_amount_pesewas = ceil(usdc × rate / 1e6)` (checked,
  > 0, ≤ u64), `pay_deadline = now + t_pay`, no self-buy. Locks ONLY via `lock_for_buy`, which now
  also checks quote min/max, the new-buyer per-order cap and the LP's new-buyer exposure in the
  same step (PR #9 review). `Lp.new_buyer_open` tracks exposure.
- `expire_order(id)`: anyone, only AWAITING_PAYMENT and after pay_deadline + t_grace; closes ONLY
  via `unlock` once, with the stored amount and new-buyer flag.
- `Params.new_buyer_orders` (1–100, demo 3); `buyer_released` counter (incremented on release in
  M1.5). Views get_order / list_orders_by_lp / list_orders_by_buyer (≤ 50) / get_buyer_released.
- Events order_opened, order_expired. Timestamps as U64 strings in JSON (> 2^53).
- `expect(dead_code)` removed: the helpers are now used.
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 151 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.94s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --all-targets 2>&1 | grep -cE '^(warning|error)'
0
```
Failed / blocked: first compile missed an import (EscrowExt); one clippy nit in a test helper;
`lock_beyond_the_bond_fails` now locks 1 USDC instead of 1 unit, because the new quote-minimum
check (1 USDC) would otherwise fire before the bond check (test intent unchanged).
Next: reviewer subagents on the PR, fix, merge; then M1.5 verdict handling.

## 2026-11-03 — Session 3: PR #9 review fix-ups (2 reviewers, D-019)
Reviewers found no fund-safety bug. Fixed in commit 96f3acb (code + SPEC) and the following docs
commits (my docs script stopped twice on text that did not match; re-applied file by file):
- `unlock`: amount > 0, "no open orders" and "unlock exceeds locked" as separate checks (a 0-amount
  unlock would have dropped open_orders while USDC stayed locked).
- Profile names limited to A-Z a-z 0-9 - _ ; message "secret_ref must belong to the caller".
- `Pot` serializes as "bond" / "liquidity" (decided before any indexer depends on it).
- `#[cfg_attr(not(test), expect(dead_code))]` so the attribute warns once M1.4 uses the helpers.
- Tests: exact event payloads, bond withdraw event, a distinctive profile checked absent from logs
  by value, registration cost identical for 1- and 64-char profiles, 64-char boundary, free-text
  profile rejected, empty profile, unregistered caller, same-ref re-set clears key_fp, lock 0,
  unlock 0, unlock with no open orders, first pause emits.
- Mutation check: re-applied the reviewer's surviving mutations (drop profile_room charge, wrong pot
  in withdraw event, wrong max in quote event, drop lock amount > 0): each now fails a test.
- Design rules recorded: PLAN M1.4, M1.6, M2.2c (fingerprint callback + secret_gen), M3.1 (sell
  orders count in open_orders). SPEC §3.6. THREAT_MODEL T19/T28/T34 tests. DECISIONS D-019.
```
$ cd contracts && cargo test
test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.24s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
$ cargo clippy --all-targets 2>&1 | grep -cE '^(warning|error)'
0
```
Next: merge PR #9 after checks; M1.4 buy orders.

## 2026-11-03 — Session 3: M1.3 DONE (events, provider limit, secret lock, bond rule, pause)
Branch: `escrow/events-provider-and-bond-lock`. Process (Kwame, Nov 3): Claude merges PRs after
tests + checks pass and reviewer subagents have reviewed; Kwame reviews afterwards.
Done (`src/events.rs`, `src/lp.rs`, `src/lp_m13_tests.rs`):
- NEP-297 events (standard "openramp" 1.0.0) for every LP action; tests check no secret profile
  appears in the logs.
- Provider exactly "paystack" | "pawapay" (D-017). Two older tests kept; their expected message
  changed because the rule deliberately changed (1-32 chars → exact names).
- `set_secret_ref` (1 yocto, zero open orders, own secret only, clears key_fp); registration now
  also reserves storage for a 64-char profile; test proves the growth stays within what was paid.
- `pause_lp` / `resume_lp` (1 yocto, event only on change).
- Internal `lock_for_buy` (paystack LP, not paused, key_fp set, liquidity, bond rule
  locked + amount ≤ bond) and `unlock` (back to liquidity or released), for M1.4.
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 109 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --all-targets   (no warnings)
$ cd escrow && cargo near build non-reproducible-wasm   → Finished cargo near build in 12s
```
Failed / blocked: first run: 2 expected failures (provider rule change); then one test bug (a new
test context resets the mock storage counter, so the storage-growth test now measures inside one
context). Fixed; no test weakened.
Next: reviewer subagents on the PR, fix findings, merge, then M1.4 buy orders.

## 2026-11-03 — Session 3: design decisions before M1.4 + Q5 sandbox proof (docs only)
Branch: `docs/design-decisions-before-m1-4` (from main after PR #7).
Q5 proof (`python3 scripts/test-b/pawapay_metadata_probe.py`, raw output, phone masked):
```
SENT salt: 786047a809a212f7ea28d81221aac3d7a2b14c2b30d9a96aae611113d5e67475
INITIATE 200 {"payoutId":"887bab2b-7ee9-4b75-8b4c-c3f70331daef","status":"ACCEPTED","created":"2026-11-03T19:32:53Z"}
STATUS 200 RAW (phone masked):
{
  "data": {
    "payoutId": "887bab2b-7ee9-4b75-8b4c-c3f70331daef",
    "status": "COMPLETED",
    "amount": "5.00",
    "currency": "GHS",
    "country": "GHA",
    "recipient": {
      "type": "MMO",
      "accountDetails": {
        "phoneNumber": "*********789",
        "provider": "MTN_MOMO_GHA"
      }
    },
    "customerMessage": "nearmomop2p",
    "created": "2026-11-03T19:32:53Z",
    "providerTransactionId": "5c2b2588-4c8d-44c4-bb47-5ede8b52ad9d",
    "metadata": {
      "openrampSalt": "786047a809a212f7ea28d81221aac3d7a2b14c2b30d9a96aae611113d5e67475"
    }
  },
  "status": "FOUND"
}
salt returned in clear and identical: True
```
Note: metadata is a LIST in the request but an OBJECT in the reply, and `isPII` is not echoed.
Done (docs, Kwame agreed): D-017 (pending = outage up to T_outage_max, one provider per LP,
checkout via escrow, ceil rounding, new-buyer definition, claimable balances, NEP-145 for sell
storage, 401-only LP_FAULT, agents track dropped, inv. 6 is ≥), D-018 (Q5 design + evidence).
SPEC v0.4 (§2, §3.1, §4.1–4.5, §5 recipient commitment rules 1–5, §6, §7 `new_buyer_orders`,
inv. 2/6/13, Q2/Q5/Q6 answered, stale Paystack-transfer text replaced). THREAT_MODEL T5/T23/T25
updated, T35/T36 added. PLAN: [must]/[nice] labels; M1.3 starts with NEP-297 events + provider
limit; M1.4/M1.5/M1.7/M2.1b updated; new M2.2b, M2.8, M2.9, M3.0, M3.4; M5.3/M5.5. AGENTS.md +
CLAUDE.md: approved diff applied (identical files).
Failed / blocked: none. No contract code changed.
Next: M1.3 (events + provider limit first).

## 2026-11-03 — Session 3: PR #7 fix-up after 3-reviewer review (D-016)
Done:
- `update_quote` requires 1 yocto (T34). `on_withdraw` checks predecessor in its body.
- Withdraw gas: `ft_transfer` 20 TGas min + all unused gas (weight 1); `on_withdraw` 15 TGas,
  weight 0. Test asserts the created receipts (receiver, args, deposit, gas, weight).
- `Lp.key_fp: Option<String>`; registration charges +68 bytes for it.
- `new`: `min_failed_attempts` 2–10, OutLayer gas ≤ 250, `new_buyer_cap` > 0, ceilings on timing
  and fee. Existing test `rejects_zero_failed_attempts` kept; its expected message changed
  because the rule deliberately changed (≥1 → 2–10). `rejects_total_gas_over_limit` now uses
  250+31 so it still tests the sum, not the new 250 cap. Two update_quote tests now attach 1 yocto.
- 30 new tests: receipts for withdrawals, Bond-pot callback, ft_on_transfer returns 0, stranger
  callback, bond locked with an open order, 1-yocto edges (0, 2), zero withdraw, name/profile
  lengths, credit overflow, exact storage cost (no refund), cost − 1 yocto fails, refund on overpay.
- mock-ft: 1,000 mUSDC per-mint cap (+2 tests); clippy nit fixed. clippy installed for 1.97.0.
```
$ cd contracts && cargo test
     Running unittests src/lib.rs (target/debug/deps/openramp_escrow-d81469ffa25a38a7)
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.90s
     Running unittests src/lib.rs (target/debug/deps/openramp_mock_ft-f436da86f5147f23)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
$ cargo clippy --all-targets   (no warnings printed)
$ cd escrow && cargo near build non-reproducible-wasm   → Finished cargo near build in 15s
```
Failed / blocked: first runs failed as expected after the rule changes (3 tests), then a type
mismatch (mock GasWeight) and a test-setup issue (mocked storage persists within a test, so the
storage-cost probe uses a same-length second LP account). All fixed; no test weakened.
Next: docs PR with the design decisions Kwame agreed (pending = outage up to the hard limit,
one provider per LP, checkout through the escrow, undefined rules, Q5 proposal, 403 = outage),
must-have / nice-to-have plan list, AGENTS.md diff for approval. Then M1.3.

## 2026-11-03 — Session 3: M1.2 DONE (LP registration, deposits, withdrawals)
Branch: `escrow/lp-bonds-and-deposits` (from main).
Done: `contracts/escrow/src/lp.rs` + `lp_tests.rs`:
- `register_lp(provider, secret_ref, quote)`: caller pays the storage of its record (refund of the
  excess), rejects double registration, bad quotes (rate 0, min 0, min > max), bad names, and a
  `secret_ref` that is not the LP's own (new T33, found in self-review).
- `update_quote` (new orders only). `get_lp` view.
- `ft_on_transfer(sender, amount, msg)`: only the configured token (inv. 9, T26); msg "bond" or
  "liquidity"; amount > 0; sender must be a registered LP. Every rejection panics so NEP-141
  refunds the full amount.
- `withdraw_liquidity` / `withdraw_bond`: exactly 1 yoctoNEAR, only available balance, bond only
  with zero open orders (inv. 4; exercised with real orders in M1.3). Debit first, `ft_transfer`,
  then `on_withdraw` (#[private]) re-credits if the transfer failed.
```
cargo test (contracts/): escrow 51 passed (28 config + 23 LP); mock-ft 3 passed; 0 failed, 0 warnings
cargo near build non-reproducible-wasm (escrow): OK (local only, not reproducible; D-015)
```
Failed / blocked: none. First run had 4 must-use warnings (detached refund promise, test helpers);
fixed with `.detach()` (near-sdk 5.30 `Promise::detach`).
Next: M1.3 bond/secret lock while orders open, bond rule, `pause_lp`.
## 2026-11-03 — Session 3: M0.6b Test B2 DONE (pawaPay sandbox payouts)
Branch: `test-b2/pawapay-sandbox-payouts` (from main after PR #5).
Setup check: Kwame's sandbox token works through the proxy (network secret; no env var).
`GET /v2/wallet-balances` 200, company "near-momo-p2p", GHA wallet GHS 10000, Ghana MTN /
Vodafone / AirtelTigo OPERATIONAL for payout + deposit (limits GHS 1–15000 / 1–10000).
`python3 scripts/test-b/pawapay_payouts.py` → exit 0:
```
PASS config: HTTP 200, MTN_MOMO_GHA GHS PAYOUT=OPERATIONAL, GHA providers=['AIRTELTIGO_GHA', 'MTN_MOMO_GHA', 'VODAFONE_GHA']
PASS wallet: GHA GHS balance before = 10000
PASS initiate completed: HTTP 200, status=ACCEPTED, payoutId=e97a215e-4a39-4c4b-8563-e458f82582a4, to ***789, failure=None
PASS initiate failed: HTTP 200, status=ACCEPTED, payoutId=3d1a6ac3-f7ea-4d80-a7c7-a5217160105e, to ***089, failure=None
PASS initiate pending: HTTP 200, status=ACCEPTED, payoutId=85c4834a-0231-4148-aaf6-ab902137171d, to ***129, failure=None
PASS duplicate payoutId: HTTP 200, status=DUPLICATE_IGNORED
PASS status completed: found=FOUND, status=COMPLETED, failure=None, amount=5.00 GHS, recipient ***789 MTN_MOMO_GHA (matches sent: True)
PASS status failed: found=FOUND, status=FAILED, failure=RECIPIENT_NOT_FOUND, amount=5.00 GHS, recipient ***089 MTN_MOMO_GHA (matches sent: True)
PASS status pending: found=FOUND, status=IN_RECONCILIATION, failure=None, amount=5.00 GHS, recipient ***129 MTN_MOMO_GHA (matches sent: True)
PASS status unknown id: HTTP 200, status=NOT_FOUND
INFO wallet GHA GHS: before=10000, after=9989.9
```
Findings:
- Status returns recipient `phoneNumber` + `provider`: the T23 recipient check is feasible.
- Reused payoutId → `DUPLICATE_IGNORED` (replay protection on pawaPay's side too).
- The docs' "SUBMITTED" number 233593456129 actually goes to `IN_RECONCILIATION` → maps to
  UNVERIFIABLE / OUTAGE: a real trigger for the sell-flow "provider stuck, window extended" demo.
- Amount echoed as "5.00" for "5": compare as numbers (SPEC §5 updated).
- Wallet dropped 10.1 GHS: 5 completed + 5 held in reconciliation + 0.1 fees; failed payout not charged.
Failed / blocked: none. Korapay fallback not needed.
Next: M1.2 LP registration, bond and liquidity deposits.

## 2026-11-03 — Session 3: M1.1 fix-up after review of PR #5
Done (each change has its reason in the code comment or DECISIONS):
- No owner role (D-014): removed `owner`; all params fixed at init; SPEC §2, §7, inv. 8–9, T20, T26.
- `new` callable only by the contract account (T32, found in this review): explicit `require!`
  in the body, because `#[private]` is enforced only in the wasm entry point and a unit test
  calling `new` directly did NOT panic (the test caught that). PLAN M2.7: deploy + init in one tx.
- `Params.outlayer_call` (inv. 11): fee, gas 100–250 TGas, callback ≥ 10 TGas, total ≤ 280,
  instructions ≤ 500e9, memory ≤ 4096 MB, seconds ≤ 180 (OutLayer docs maximums).
- Init rejects token = outlayer, token/outlayer = the contract itself.
- deploy_tag formula `sha256(contract_id ":" height)` now identical in code, SPEC §4.1, T27, with a
  test against the literal formula.
- Tests added: zero t_pay / t_retry / fee, low/high gas, instruction/memory/second bounds and
  maximums, empty Project id/version, valid Project, UPPERCASE 64-char hash, non-hex 64-char hash.
- mock-ft: minting to a NEW account requires the storage deposit (stops draining its NEAR);
  `ft_resolve_transfer` emits FtBurn when a refund is burned. 2 new tests.
- `rust-toolchain.toml` (1.97.0), reproducible-build metadata in both Cargo.toml files (D-015),
  README build note. Old local hashes above labelled "not reproducible".
```
cargo test (contracts/): escrow 28 passed; mock-ft 3 passed; 0 failed, 0 warnings
```
Failed / blocked: first run had 1 failure (`init_by_another_account_is_rejected`: `#[private]` not
visible to unit tests); fixed by the explicit check above.
Reproducible build attempt (commit c3e90b5, `cargo near build reproducible-wasm`, dockerd started
in the container): git clean check + image digest OK, then `docker image pull
sourcescan/cargo-near:0.22.0-rust-1.97.0@sha256:b193c864…` → Docker Hub `429 Too Many Requests`
(anonymous pull rate limit). Tried twice; stopped per the 2-attempt rule. Not a config problem.
Next: retry the reproducible build in a later session (before M2.7 deploy, PLAN M2.7); if Docker
Hub keeps rate-limiting, Kwame adds Docker Hub credentials as a secret. Then Test B2 or M1.2.

## 2026-11-03 — Session 3: M1.1 DONE (escrow scaffold + mock USDC)
Branch: `escrow/scaffold-and-mock-usdc` (from main after PR #4).
Done: `contracts/` cargo workspace (shared release profile).
- `contracts/escrow`: `new(owner, token, outlayer, verifier, params)` stores the deploy-time config
  with no setters (inv. 9); `deploy_tag` = first 8 hex of sha256(contract_id ":" block_height)
  (inv. 12, T27); validates verifier (https URL + 64-hex sha256, or project+version) and params
  (t_outage_max >= t_retry, min_failed_attempts >= 1, outlayer_fee > 0); `get_config` view.
- `contracts/mock-ft`: TEST ONLY NEP-141 mUSDC, 6 decimals, open `mint` (D-013).
```
cargo test (contracts/):
  escrow: 6 passed (init_stores_fixed_config, deploy_tag_is_8_hex_and_differs_per_deployment,
          rejects_bad_verifier_hash, rejects_http_verifier_url,
          rejects_outage_window_shorter_than_retry, rejects_zero_failed_attempts)
  mock-ft: 1 passed (mint_and_transfer)
cargo near build non-reproducible-wasm (LOCAL ONLY, not reproducible, do not quote; see D-015):
  openramp_escrow.wasm  115750 B  sha256 a9de3590726ccb5f18be1bdfc5aa1f5c0e79699cab1cc1a245f4f38327782f26
  openramp_mock_ft.wasm 146163 B  sha256 253a471face17da507740f049d45cbbc246235fc462c01f76ee5d26ea42330e6
```
Failed / blocked: none.
Next: M1.2 LP registration, bond and liquidity deposits via `ft_transfer_call`, withdrawals.
Gotchas: commits no longer carry the Claude co-author / session trailers (Kwame, Nov 3).

## 2026-11-02 — Session 2: pawaPay decision + slash timing (docs only)
Branch: `docs/pawapay-decision-and-slash-rule` (from main after PR #3).
Done: D-011 (pawaPay sandbox for sell payouts, demo only; production = licensed LPs; Korapay
fallback; pawaPay deposits deferred). D-012 (failure classes LP_FAULT / OUTAGE; T_retry 24 h,
T_outage_max 72 h, demo 3 / 6 min; every failure still ends in a slash; reasoning recorded).
SPEC v0.3 (§4.5 classes table, §5 pawaPay specifics: UUID payout ids, canonical 233 numbers,
string amounts, status mapping; §7, inv. 11, §9), THREAT_MODEL T31, PLAN M0.6b (Test B2),
M1.6, M3.1/3.2, M4.6 (four demo outcomes incl. provider-down extension), M5.1; README;
.env.example. Checked: pawaPay sandbox needs no onboarding (how_to_start); prohibited list bans
"unlicensed … money transfer".
Failed / blocked: none. No code changed.
Next: [K] pawaPay sandbox token → Test B2. Meanwhile M1.1 escrow scaffold.

## 2026-11-01 — Session 2: M0.6 closed with decision D-010 (buy flow is the main demo)
Done: Kwame chose option 3. Recorded D-010; PLAN M0.6 ticked (charge PASS, transfers blocked),
M3.1/M3.2 use a `SimulatedPayout` provider in a separate, labelled verifier build, S1 dropped,
M5.1 README item added; SPEC §5 note, §9 and Q2 updated; AGENTS.md/CLAUDE.md agent-api line;
README status lines. No code changed.
Failed / blocked: none.
Next: M0.7 [K] fund mainnet dev account (~$3 USDC + a little NEAR) for Test C, or start M1
(escrow core) while waiting.
Gotchas: if Kwame's Paystack upgrade is approved later, rerun `scripts/test-b/run.py` (exit 0
expected) and switch M3 to real transfers.

## 2026-11-01 — Session 2: M0.6 Test B (BLOCKED: Paystack Starter account cannot send transfers)
Branch: `test-b/paystack-momo-payouts` (from main after PR #2).
Done: `scripts/test-b/run.py` (stdlib only; refuses unless Paystack says domain=test; masks phones):
```
PASS preflight: domain=test (ref opr-test-b-mode-1793571896)
PASS charge: HTTP 200, status=success, gateway=Approved, phone ***987
PASS charge verify: status=success, ref=opr-test-b-charge-1793571896, amount=1000 GHS, channel=mobile_money, paid_at=2026-11-01T22:24:57.000Z
PASS recipient: HTTP 201, RCP_ll1vjfsjqk79ot2, number ***987, bank MTN
BLOCKED transfer: HTTP 400, code=transfer_unavailable, message='You cannot initiate third party payouts as a starter business'
INFO transfer verify (no transfer): HTTP 404, code=not_found, message='Transfer not found'
exit 2
```
- GHS mobile money bank codes (GET /bank?currency=GHS&type=mobile_money): MTN, VOD (Vodafone/Telecel), ATL (AirtelTigo).
- Recipient object includes `details.account_number`, `bank_code` and `integration`: usable for the
  T23 recipient check, IF transfer verify returns the recipient (not yet seen: no transfer exists).
- Transfer verify "not found" is HTTP 404 `code: not_found` (payment verify uses 400
  `transaction_not_found`): the verifier must map each endpoint separately.
Failed / blocked: POST /transfer → 400 `transfer_unavailable`. Paystack help: Starter businesses
have no Transfers; upgrade to Registered Business on the Compliance page (needs business
registration docs and a matching business bank account).
https://support2.paystack.com/hc/en-us/articles/360009972719-What-is-a-Paystack-Starter-Business
Impact: buy flow (on-ramp) unaffected. Sell flow (M3) and agent-api stretch (S1) need payouts.
Next: Kwame decides (upgrade / other payout provider / sell flow with mocked payouts).

## 2026-11-01 — Session 2: M0.10 security design update (docs only)
Done: Reviewed Kwame's security audit (written before M0.4/M0.5). Already handled: #5 secret
access (M0.5), #9 callback (M0.4), #10 (the real verifier fails closed). Adopted #1, #2, #4, #6, #7,
#8 as SPEC v0.2 invariants 9–14 + THREAT_MODEL T23–T29; #3 adopted with a change (T30: every
failure still counts, because an LP can cause failures too). #11 → PLAN M2.6. #12: moved
`claude/agents/security-auditor.md` → `.claude/agents/` (merged main first: PR #1 + agent file).
Checked: Paystack verify response has no integration id (field list only), so key fingerprint it is.
Failed / blocked: none. No code changed.
Next: M0.6 Test B (transfers + transfer verify; also check recipient fields for T23).

## 2026-11-01 — Session 2: M0.5 DONE (Test A complete: real Paystack verdicts on OutLayer)
Done: Kwame stored the real test key as OutLayer secret profile `paystack` (wasm-hash 021f85…c61,
access allow-all via test-a.ikwame.testnet). All runs through `scripts/test-a/verify-run.sh`,
contract test-a.ikwame.testnet, results stored by the callback:
| Case | Reference | Stored verdict / reason | Tx |
|---|---|---|---|
| Real test MoMo payment | opr-test-a-paid-1 | PAID / ok | B69dGnWRqjZLGe2qn6ot5859t75jNQozdNn8rekcwWdF |
| Reference not found | opr-test-a-missing-1 | NOT_PAID / not_found | DAwdwkw24Y5Mrv5YBhUmxjyujxqWuP4dVrNpfADZHFWu |
| Abandoned (never paid) | opr-probe-modecheck-2 | NOT_PAID / not_success | 7B4Jy3Szjpm6YumSZGe6YuxmEtxrkbNxck48jkAbuRzD |
| Expected 2000, paid 1000 | opr-test-a-paid-1 | NOT_PAID / amount_short | An3aApfZ3xKKcwPM3cchdNozKRe2nXTFjyUAUCv9MmQh |
| Deadline before paid_at | opr-test-a-paid-1 | NOT_PAID / late | Bf1KiWeosu9HULw3cfGnAQ8jtarX79Q428ofpQj3SgbX |
| Invalid key (profile bad-key) | opr-test-a-paid-1 | UNVERIFIABLE / auth_error | 6ZzC8kmJtBXCJ3s4EbRGk4DoJJjaUqXuauh7jSc1wuMd |
| Real key called directly (not via contract) | — | refused: "Access denied by access condition" | vxWy9hD7zLdbPM6on1AR89kCXZ3Za4MrouXb4JQKPHn |
Test A (SPEC Q1) is answered: call shape, deposit (~0.003 NEAR estimate; 0.001 base fee kept on
failure), callback format, secret access, who can deliver the callback.
Failed / blocked: none.
Next: M0.6 Test B: Paystack test-mode mobile money transfer + transfer verify (the charge half
is already proven above). Then M0.7 [K] fund mainnet dev account, M0.8 Test C.
Gotchas: wasm outbound HTTPS works without any allowlist (no connector manifest). Paystack
`paid_at` is ISO 8601 with milliseconds and `Z`.

## 2026-11-01 — Session 2: M0.5 Test A part 3 (IN PROGRESS: waiting for Kwame to store the real key)
Done:
- Paystack key is TEST mode: `/transaction/initialize` (ref opr-probe-modecheck-2) then verify →
  `domain: test`, status `abandoned`. Custom `opr-…` references accepted.
- PAID fixture: `POST /charge` GHS 1000 pesewas, `mobile_money {phone 0551234987, provider mtn}`,
  reference `opr-test-a-paid-1` → `success` / `Approved`, channel mobile_money, paid_at
  2026-11-01T20:48:20Z (answers the charge half of SPEC Q2).
- `scripts/test-a/paystack-verify`: pure `decide()` + `reference_is_valid()` in lib.rs, wasi-http
  call in main.rs. `cargo test`: 15 passed. wasm sha256
  021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61, hosted in `wasm/` (commit 14ff01b).
- test-a contract: `request()` now takes optional `secrets_ref`; 6 tests pass; redeployed
  (tx 5GETLALTzZK7j1o3MLc4AjFSKMPGBVHY3sVybvmCV25G).
- `scripts/test-a/verify-run.sh <profile> <reference>` runs one check through the contract.
- UNVERIFIABLE (revoked/invalid key): secret profile `bad-key` = `sk_test_000…` (not a real key),
  bound to the wasm hash, access `allow-all --via test-a.ikwame.testnet` →
  `verify-run.sh bad-key opr-test-a-paid-1` → stored
  `{"verdict":"UNVERIFIABLE","reason":"auth_error"}` (tx 6ZzC8kmJtBXCJ3s4EbRGk4DoJJjaUqXuauh7jSc1wuMd).
- Access control works: same secret called directly by ikwame.testnet (not via the contract) →
  "Access denied by access condition — signer ikwame.testnet, called from ikwame.testnet"
  (tx BLA7e4tvgUPgWuHxCgVccHLtdRmuinHfx9SuGMh4P8z4).
Failed: first try used `--access whitelist:test-a… --direct` → denied, because a Whitelist is
checked against the SIGNER (ikwame.testnet). The right rule is `--access allow-all --via <contract>`
= `Predecessor{Whitelist[contract]}`: any signer, but only through that contract. For the escrow
this is what each LP stores (plus the build lock).
Next: [K] store the real test key as profile `paystack` with the same binding/access, then run
PAID (`opr-test-a-paid-1`) and NOT_PAID (`opr-test-a-missing-1`, and `opr-probe-modecheck-2` = abandoned).
Gotchas: `outlayer secrets list` shows nothing for wasm-hash rows. Every verifier rebuild changes
the hash, so the secret must be re-bound (`outlayer secrets access --build`) or re-stored.

## 2026-11-01 — Session 2: M0.4 DONE (contract → OutLayer → callback)
Done:
- `scripts/test-a/contract` (near-sdk 5.30): `request(wasm_url, wasm_hash, input)` calls
  `outlayer.testnet.request_execution` (200 TGas, deposit = attached) `.then(on_result)` (30 TGas,
  `#[private]`); `on_result` sorts the result with `classify()` and stores it; `get_records` view.
  `cargo test`: 6 passed (output / null / empty / not-JSON / too long / promise failed).
  `cargo near build non-reproducible-wasm` OK. Deployed to test-a.ikwame.testnet (3 NEAR,
  create tx Dd4nhjpQ1QS3K6amK6yNqq47z7pUmZ4PS1BdCkUmKv6U, deploy tx 3y9YaotkF3gbYS4batLRueKQghFhT7AZYcmXzaPhdkdH).
- Run 1, real hello.wasm → stored `{"kind":"Output","value":{"input":{"command":"hello","order":"opr-7"},
  "message":"Hello, ikwame.testnet!","success":true}}` (block 275800166). Attached 0.01 NEAR;
  estimate_execution_cost said 0.0030001 NEAR.
- Run 2, wrong hash (00…00) → OutLayer `"wasm_url content does not match its hash"`, charged
  0.001 NEAR, refunded 0.009 → stored `{"kind":"RunFailed"}` (tx CD1r8t2AmjYsX7Keyt1pRfRvWrJfcSidsxgWvEHoMSMW).
- Run 3, ikwame.testnet calls `on_result` directly → `Smart contract panicked: Method on_result is
  private`; no record added (T13 confirmed).
- `near view test-a.ikwame.testnet get_records` → exactly the 2 records above.
- SPEC Q1 answers: call shape and deposit as above; our contract owns the `.then` callback,
  OutLayer owns the yield (it resumes its own `on_execution_response`, whose return value is what
  our callback gets). Result = program JSON on success, `null` on failed run, empty on promise
  failure / timeout. Only the OutLayer operator can resolve a request (`assert_operator`).
  `NEAR_SENDER_ID` in the wasm = the transaction signer (ikwame.testnet), not the calling contract.
  Source: github.com/out-layer/outlayer contract/src/execution.rs, app.outlayer.ai/docs/near-integration.
Findings for the escrow (added to THREAT_MODEL): T21: the escrow must fix the verifier wasm
URL+hash itself, never take it from the caller. T22: operator trust, same as T17.
Failed / blocked: none. Timeout path not triggered live (can't force OutLayer to stay silent);
covered by reading the source + unit test.
Next: M0.5 Test A part 3: wasm calls Paystack with the key from an OutLayer secret; PAID /
NOT_PAID / UNVERIFIABLE. Needs Kwame to store the Paystack test key as an OutLayer secret.
Gotchas: `PromiseError` is non_exhaustive (needs a catch-all arm; fail closed). Use
`promise_result_checked` (old `promise_result` is deprecated). near CLI echoes the private key
in its "re-run" hint: always filter output.

## 2026-11-01 — Session 2: M0.3 DONE (hello-world OutLayer run from the CLI)
Done:
- Repo is already PUBLIC (Kwame checked Settings; earlier notes saying "private" are outdated).
- Hosted the wasm in `wasm/hello.wasm` (D-008). Raw URL pinned to commit 2b53680, downloaded sha256
  matches 0ca1af08…a36b.
- `outlayer run --wasm <raw url> --hash 0ca1af08… --input in.json` → tx
  Gos57Sqjrf4RX3vS8KbRWQe2PyiDgLZDn6RkbupkWL5J, output:
  ```
  {"input": {"command": "hello", "order": "opr-1"}, "message": "Hello, ikwame.testnet!", "success": true}
  ```
  Echo verified: the input comes back unchanged, through OutLayer's on-chain callback.
Failed / blocked:
- `outlayer deploy hello <raw url>` → "Version '0ca1af08…' already exists in project": versions
  are keyed by hash, and that hash is still bound to the dead FastFS URL. Used `--wasm` instead.
- With `--wasm`, a positional JSON arg is read as PROJECT, not INPUT (got `"input": {}`). Use
  `--input <file>`.
- test.fastfs.io still 502 at end of session.
Next: M0.4 Test A part 2: tiny testnet contract calls `request_execution` and stores the callback
result (call shape already recorded in the M0.3 part 1 entry below).
Gotchas: `NEAR_SENDER_ID` inside the wasm = the account that signed the request (here
ikwame.testnet); in M0.4 check whether it becomes the calling contract.

## 2026-11-01 — Session 2: M0.3 continued (BLOCKED: FastFS testnet gateway down)
Done:
- Funding: near-faucet.io failed for Kwame ("There was an error", 0 NEAR arrived). Workaround:
  `near account create-account sponsor-by-faucet-service openramp-funder-64532.testnet
  use-manually-provided-public-key <ikwame.testnet public key>` (got 10 NEAR, same key, no new
  secret), then sent 9.9 NEAR → ikwame.testnet. Balance now 9.9 NEAR.
- `outlayer upload out/hello.wasm` OK →
  https://test.fastfs.io/ikwame.testnet/outlayer.testnet/0ca1af08790c4d29256766cb7193840c908339e3353221e397418814cdfaa36b.wasm
- `outlayer deploy hello <url> --hash 0ca1af08…a36b` → "Project deployed" (without `--hash` the CLI
  tries to download the file and fails on the 502).
- `outlayer run ikwame.testnet/hello '{"command":"hello"}'` → tx
  BVDybc2yosHq9ZNUHRTJiCfbEyjvbknBjUzsXDiMWdna. The on-chain round trip WORKS:
  `execution_requested` (request_id 6435) → yield → `execution_completed` callback on
  outlayer.testnet, charged 0.001 NEAR then refunded. Final tx value: `null`.
Failed / blocked:
- Worker log: `Execution failed: "wasm_url could not be fetched"`. `curl https://test.fastfs.io/`
  (gateway root, not just our file) → `502 error code: 502` (Cloudflare). FastFS testnet is down.
  Attempt 1: deploy without --hash (502). Attempt 2: deploy with --hash + run (worker can't fetch).
- Docs (https://app.outlayer.ai/docs/getting-started): "Any https URL works for `outlayer deploy`";
  the worker checks the sha256. So a public HTTPS host for the wasm is a valid fallback.
Next (Kwame to choose): (a) retry later when test.fastfs.io is back, or (b) host hello.wasm at a
public HTTPS URL (our repo is private, so raw.githubusercontent won't work without making it
public; e.g. a release asset in a small public repo).
Gotchas: an execution failure is still delivered through the callback (with the fee refunded),
so the escrow must treat an OutLayer error as UNVERIFIABLE (SPEC §4.5 already says so). Set
RUST_BACKTRACE=0 to get readable outlayer CLI errors.

## 2026-11-01 — Session 2: M0.3 Test A part 1 (BLOCKED: testnet account has 0 NEAR)
Done:
- `outlayer login testnet` with NEAR_ACCOUNT_ID / NEAR_PRIVATE_KEY piped from env (key saved to
  ~/.outlayer, outside the repo). `outlayer whoami` → ikwame.testnet, contract outlayer.testnet.
- `outlayer create hello` → `scripts/test-a/hello` (template already echoes the JSON input).
  `./build.sh` → `out/hello.wasm`, 173717 bytes,
  sha256 0ca1af08790c4d29256766cb7193840c908339e3353221e397418814cdfaa36b.
- From outlayer-cli 0.2.1 source (`src/commands/run.rs`), the on-chain call (useful for M0.4):
  `outlayer.testnet.request_execution({source: {Project:{project_id,version_key}} |
  {WasmUrl:{url,hash,build_target}}, resource_limits:{max_instructions,max_memory_mb,
  max_execution_seconds}, input_data:<JSON string>, secrets_ref:{profile,account_id}|null,
  response_format:"Json", payer_account_id:null, params:null})`, 300 TGas, deposit =
  `estimate_execution_cost(resource_limits, input_len)` view call (yoctoNEAR).
  Without PAYMENT_KEY, `outlayer run` uses this on-chain path.
Failed / blocked:
- RPC view_account ikwame.testnet: `amount 0`, 1 full-access key (matches env key). Upload
  (FastFS tx), deploy and run all need testnet NEAR. [K] fund the account, then rerun:
  `outlayer upload out/hello.wasm` → `outlayer deploy hello <url>` →
  `outlayer run ikwame.testnet/hello '{"command":"hello"}'`.
Next: after funding, finish M0.3 (paste run output), then M0.4.
Gotchas: `outlayer` commands all need `outlayer login` first (even `create`). The OutLayer
getting-started page says "Live on NEAR Mainnet" but the CLI supports testnet (outlayer.testnet).

## 2026-11-01 — Session 2: M0.2 toolchain
Done: toolchain installed from crates.io (cargo install --locked), all checks pass:
```
rustc 1.97.0 (2d8144b78 2026-07-07)
targets: wasm32-unknown-unknown, wasm32-wasip2, x86_64-unknown-linux-gnu
cargo-near-near 0.22.0
near-cli-rs 0.30.1
outlayer 0.2.1   (outlayer --help OK)
```
Recipe that works in the cloud container:
```
rustup target add wasm32-unknown-unknown wasm32-wasip2
cargo install --locked outlayer-cli
cargo install --locked cargo-near --no-default-features
cargo install --locked near-cli-rs --no-default-features --features inspect_contract,verify_contract
```
Failed / blocked:
- OutLayer's official `install.sh` (https://outlayer.ai/docs/getting-started) was blocked by the
  auto-mode safety check; the crates.io `outlayer-cli` points to the same repo (out-layer/outlayer-cli).
- api.github.com returned 403 through the proxy, so GitHub release binaries were not used.
- First cargo-near build failed: default `ledger` feature needs system `libudev` (not installed).
  Ledger is not needed, so built without default features.
Next: M0.3 Test A part 1 (hello-world OutLayer WASM). Needs `outlayer login` with the testnet key.
Gotchas: containers start clean; all of the above is lost. Full build ~10 min. See PLAN M0.2b.

## 2026-11-01 — Session 2: secrets check (handoff task 1)
Done:
- Env vars (checked set/missing only, values never printed):
  `PAYSTACK_SECRET_KEY missing`, `NEAR_ACCOUNT_ID set` (a `.testnet` account),
  `NEAR_PRIVATE_KEY set` (`ed25519:` format), `ONECLICK_API_KEY missing`.
- Paystack key is injected by the agent proxy, not an env var:
  `GET /transaction/verify/opr-probe-does-not-exist` → HTTP 400 `transaction_not_found`
  (would be 401 without auth); `GET /transaction?perPage=1` → `status True`, 0 transactions.
- Documented both mechanisms in `.env.example`.
Failed / blocked: none.
Open: not yet proven that the injected key is a TEST key (no field says so; account has no
transactions). Confirm in M0.6 by checking `domain == "test"` on the first charge.
M0.1 still open: mainnet dev account + 1Click key (for M0.7–M0.8) [K].
Next: M0.2 toolchain from scratch (cargo-near, near-cli-rs, outlayer CLI), then M0.3.
Gotchas: Paystack code must not fail when PAYSTACK_SECRET_KEY is empty in cloud sessions (proxy
adds auth). The OutLayer TEE will NOT have this proxy, so the verifier still needs the key as an
OutLayer secret (M0.5).

## 2026-11-01 — Handoff: end of session 1 (switching to a new session so secrets load)
Current state: Docs only (AGENTS, SPEC, THREAT_MODEL, PLAN, DECISIONS, PROMPTS). Kwame APPROVED
SPEC + PLAN. No code yet.
M0.1 status: Kwame created a Paystack test account and a NEAR testnet account (Meteor Wallet) and
added the keys to the cloud environment as env vars / Network secrets. Not yet done: NEAR mainnet
dev account (~$3 USDC + a little NEAR) and 1Click API key (needed only for Test C, M0.7–M0.8).
Secrets were NOT visible in session 1 (container started before they were added).
In progress: M0.2 toolchain. In session 1 only `rustup target add wasm32-unknown-unknown
wasm32-wasip2` ran; containers start clean, so redo M0.2 from scratch.
Next 3 tasks:
1. Check which secrets are visible without printing values:
   `for v in PAYSTACK_SECRET_KEY NEAR_ACCOUNT_ID NEAR_PRIVATE_KEY; do [ -n "${!v}" ] && echo "$v set" || echo "$v missing"; done`
   If missing, they may be Network secrets injected by the proxy into requests instead of env vars:
   probe with `curl -sS https://api.paystack.co/bank?country=ghana -o /dev/null -w '%{http_code}'`
   (200 = auth injected, 401 = not). Record which mechanism works in PROGRESS.md and .env.example.
2. M0.2 toolchain: wasm targets, cargo-near, near-cli-rs, outlayer CLI (install steps:
   https://outlayer.ai/docs/getting-started). Check versions.
3. M0.3 Test A part 1: hello-world OutLayer WASM run from the CLI.
Gotchas: Rust 1.97 and Node 22 are preinstalled. Outbound HTTPS goes through a proxy (CA bundle
/root/.ccr/ca-bundle.crt). Kwame prefers plain, short explanations; he got lost once, so state
next steps simply.

## 2026-11-01 — Session 1: project setup (kickoff)
Done:
- Repo created (`Ikwame91/near-momo-p2p`, private). Name chosen: OpenRamp (D-005).
- Wrote AGENTS.md + CLAUDE.md (copy), docs/SPEC.md, THREAT_MODEL.md, PLAN.md, DECISIONS.md,
  PROMPTS.md, .gitignore, .env.example, LICENSE (MIT, required by hackathon rules).
- Checked HACKATHON.md: repo must be public by Nov 14 23:59 UTC, LICENSE required, README with
  "do X → see Y", X post with video, Intents is mainnet-only, AI judges (Claude Code, Codex) screen
  reproducibility.
- OutLayer docs (https://app.outlayer.ai/docs): call `request_execution` on `outlayer.testnet`,
  result via yield/resume callback, secrets injected as env vars, build `wasm32-wasip2`.
Failed / blocked: none. Feasibility tests A, B, C have NOT been run yet.
Next: Kwame approves SPEC + PLAN → M0.1 accounts [K] and M0.2 toolchain → M0.3 Test A.
Gotchas: mainnet OutLayer account id not on the docs index page; full `request_execution` schema is
on https://outlayer.ai/docs/near-integration.
