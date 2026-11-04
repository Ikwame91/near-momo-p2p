# OpenRamp: Plan

Status: v0.2 (Nov 4, 2026, Kwame): compressed schedule, buy flow only. Each task ≤ 2 h and has a
check. `[K]` = Kwame must do it by hand (accounts, keys, funding).

## Schedule (Kwame, Nov 4) — this overrides the milestone dates below
| Days | Goal |
|---|---|
| Wed Nov 4 – Fri Nov 6 | **Build done.** Buy flow only, end to end on testnet with real OutLayer + Paystack test mode. Order: M1.7 (one books test) → OutLayer wiring (M2.1, M2.1b, M2.2, M2.2b, M2.2c) → testnet deploy early (M2.8, M2.7) → created→PAID (M2.3) and slash (M2.4) shown to Kwame as soon as each works → web page (M4.1–M4.3) → README draft (M5.1). |
| **Thu Nov 5, midday** | **Checkpoint:** if the real OutLayer → Paystack check is not working, tell Kwame immediately (do not keep pushing quietly). |
| Sat Nov 7 – Sun Nov 8 | Polish and iteration. |
| Mon Nov 9 – Tue Nov 10 | Full testing and fixes: M1.8 sandbox tests, M1.9 immutability tests, M5.2 fresh-clone run, M2.5 hostile review. |
| **Wed Nov 11** | **Submit** (buffer before the Nov 14 deadline): M5.3–M5.5. |

**Cut (buy flow only, Kwame Nov 4):** all of M3 (sell flow code: keep the pawaPay sandbox proof and
design as "designed, tested in sandbox, not built"), M0.7/M0.8 Test C and M3.3 Intents settlement,
M3.4 keeper, M1.10, M2.6 reproducible-build CI work beyond the pinned toolchain, M2.9, M4.4. M4.6
demo trimmed to PAID, NOT_PAID and slash. D-022 stays (Kwame).
Process: subagent reviews paused (D-019 update); Claude self-reviews with tests + mutation checks.

## M0 Feasibility (Nov 1–3): prove the three risky integrations
- [ ] M0.1 [K] Accounts: Paystack test account (GH), NEAR testnet account, OutLayer CLI login,
      NEAR mainnet dev account for Intents. Check: keys in local `.env` / cloud secrets, never committed.
- [x] M0.2 Toolchain: Rust + `wasm32-unknown-unknown` + `wasm32-wasip2`, cargo-near, near-cli-rs,
      outlayer CLI. Check: `cargo near --version`, `outlayer --help` run.
- [ ] M0.2b Toolchain setup script (`scripts/setup-toolchain.sh`, recipe in PROGRESS session 2) and
      add it to the cloud environment setup script [K], so new containers don't rebuild for ~10 min.
      Check: fresh container has all four versions.
- [x] M0.3 Test A part 1: hello-world OutLayer WASM (echo input) uploaded and run from CLI.
      Check: CLI shows output.
- [x] M0.4 Test A part 2: tiny testnet contract `scripts/test-a/contract` calls OutLayer
      `request_execution` and stores the callback result. Check: `near view` shows the stored result;
      record call args, deposit, callback shape, who can call the callback (SPEC Q1, T13).
- [x] M0.5 Test A part 3: WASM calls Paystack `GET /transaction/verify/:ref` with the key from an
      OutLayer secret; contract receives PAID for a real test-mode payment, NOT_PAID for a fake ref,
      UNVERIFIABLE for a revoked key. Check: three runs, three verdicts.
- [x] M0.6 Test B (charge + verify PASS; transfers BLOCKED on Starter account → D-010): script `scripts/test-b` creates a test-mode GHS mobile money charge with a custom
      reference, verifies it, and creates + verifies a test-mode mobile money transfer. Check: script
      output (SPEC Q2).
- [x] M0.6b Test B2 pawaPay sandbox payouts (D-011). All checks PASS (Nov 3). [K] sign up, generate a sandbox API token,
      add it to the cloud environment's network secrets for api.sandbox.pawapay.io (never in chat).
      Then `scripts/test-b/pawapay_payouts.py`: GET /v2/wallet-balances first (fund with a sandbox
      deposit if empty), payouts to the Ghana COMPLETED / FAILED / pending test numbers, status
      check incl. recipient phoneNumber, NOT_FOUND id. Korapay fallback if blocked > 1 day.
      Check: script output.
- [ ] M0.7 [K] Fund mainnet dev account with ~$3 USDC + a little NEAR. (approved exception D-004)
- [ ] M0.8 Test C: script `scripts/test-c` gets a 1Click quote (dry run), then moves $1 USDC
      NEAR → Base. Check: tx hashes on both explorers, fee and time recorded (SPEC Q3).
- [x] M0.10 Security design update (audit findings): SPEC v0.2 invariants 9–14, THREAT_MODEL
      T23–T30, DECISIONS D-009, agent file moved to `.claude/agents/`. Check: docs reviewed by Kwame.
- [ ] M0.9 Go/no-go: write results in PROGRESS.md, decide SPEC Q4 in DECISIONS.md, update SPEC.

## M1 Escrow core (Nov 3–6): contract logic with unit tests, no OutLayer yet
- [x] M1.1 Scaffold `contracts/escrow` (near-sdk, cargo-near), mock USDC FT for tests. Check: build + empty test passes.
- [x] M1.2 LP registration, bond and liquidity deposits via `ft_transfer_call`, withdrawals. Check: tests.
Labels: **[must]** needed for the demo; **[nice]** only if time allows (D-017 scope review).
- [x] M1.3 [must] (first) NEP-297 events for LP actions (register, deposit, withdraw, quote change)
      with no PII; provider limited to `"paystack" | "pawapay"` (D-017). Then bond/secret lock while
      orders open (inv. 4), bond rule (inv. 5), `pause_lp` / `resume_lp` with 1 yocto (inv. 14,
      T34). Check: failing→passing tests.
- [x] M1.4 [must] (PR #9 review) `open_buy_order` locks ONLY via `lock_for_buy` and closes ONLY via
      `unlock`, exactly once per order, with the amount stored on the order; quote min/max, new-buyer
      cap and per-LP new-buyer exposure checked in the same step and reduced on close.
      Buy order open/expire, `opr-<deploy_tag>-<id>` references (inv. 12), paystack LP
      only, `key_fp` must be set, `ghs_amount` = ceil formula, new-buyer counter (`new_buyer_orders`)
      + per-order cap + per-LP exposure cap (inv. 14), order views `get_order` / `list_orders_by_lp`
      / `list_orders_by_buyer` (the web app needs them). Check: tests.
- [x] M1.5 [must] (PR #10 review) `claim` accepted while `now <= pay_deadline + t_grace` (expire is
      `>`: no gap, no overlap); `buyer_released[(lp, buyer)]` += 1 only on final RELEASED (checked
      add); release delay keyed off `order.new_buyer`; claimable-balance entries are new storage:
      reserve it or fund it from the order's pre-charge.
      Verdict handling with a typed `VerifierResult` input as the mock (no trait needed): PAID / NOT_PAID /
      UNVERIFIABLE / PENDING, replay protection, reference + `key_fp` must match, callback carries
      (order_id, attempt) and stale ones are ignored (inv. 1–3, 10, T15), `finalize_release`,
      claimable balances for failed transfers + `withdraw_claimable` (D-017). Check: tests.
- [x] M1.6 [must] (PR #11 review) an LP must not be able to dodge a due slash by racing `claim`: slash
      automatically inside the failure branch of `apply_verdict` once the window is met, and/or
      allow `slash` from VERIFYING when the conditions already held at the last failure.
- [x] M1.6 [must] (PR #9 review) the slash debits `bond` in the same internal step that unlocks the
      order; test `locked <= bond` after a slash with other orders open.
      Retry window + slash with `min_failed_attempts`, failure classes `LP_FAULT` (`T_retry`) /
      `OUTAGE` (`T_outage_max`) (T5, T8, T25, T30, T31, inv. 11, D-012). The contract-set OutLayer
      deposit/limits are already fixed in `Params.outlayer_call`; attaching them to the real call is
      M2.2. Check: tests with time travel.
- [x] M1.7 [must] Books check (inv. 6) as ONE test across all flows: contract balance ≥ owed (not =;
      stray mUSDC can be sent to the escrow). Check: test passes.
- [ ] M1.9 Immutability: no setter for verifier/token/OutLayer id; `ft_on_transfer` rejects other
      tokens (inv. 9, T26; token check + tests already done in M1.2). Check: tests.
- [ ] M1.10 [nice] Anti-griefing for experienced buyers (T7): small order fee paid to the LP on
      expiry, or a per-buyer limit on concurrent open orders.
- [ ] M1.8 (PR #12 review) also measure the gas of the slash-inside-the-verdict path, next to PAID.
- [ ] M1.8 Sandbox integration test (near-workspaces) of a full buy flow with mock verifier. Also
      prove what unit tests cannot: `#[private]` callbacks reject strangers, double init rejected,
      NEP-141 refund on a rejected deposit, failed withdrawal (receiver unregistered) is re-credited,
      and record real gas burnt for ft_transfer / on_withdraw (D-016). Check: passes.

## M2 Verifier + real wiring (Nov 6–8)
- [x] M2.1 (PR #11 review) the verifier output must carry `key_fp` and a `PENDING` verdict (Test A's has
      neither). Probe how long Paystack keeps an unapproved GH mobile-money charge pending; if it
      can stay pending indefinitely, a buyer could slash without paying (T25 via PENDING): map
      "pending older than pay_deadline + X" to NOT_PAID and record it in DECISIONS.
- [x] M2.1 (PR #10 review) exact reference format in the verifier (`opr-` + 8 lowercase hex + `-` +
      digits) and fix its stale `opr-<order_id>` doc comment.
- [x] M2.1 `verifier/` crate (start from `scripts/test-a/paystack-verify`, which already has the
      verdict logic + 15 tests): `PaymentProvider` trait, Paystack adapter, verdict mapping, unit tests
      with recorded test-mode responses. Check: tests.
- [x] M2.1b [must] (PR #12 review) verifier: map only HTTP 401 to `auth_error` (403 is an outage, test it);
      check amount ≥ expected, currency GHS and channel `mobile_money` before PAID **and** before
      PENDING; treat `paid_at ≤ pay_deadline + t_grace` as on time (the escrow passes that deadline);
      probe whether a buyer can create the Paystack transaction for our reference with the public key.
- [x] M2.1b [must] Verifier modes: `fingerprint` (returns `key_fp`, either provider), `checkout`
      (Paystack initialize: GHS, mobile_money only, synthetic email), `verify` output carries
      `key_fp`; `pending` reason; only 401 = `auth_error`, 403 and unknown = outage (D-017).
      Check: tests + testnet run.
- [x] M2.2c [must] Escrow `request_fingerprint` + callback (PR #9 review): stores `key_fp` only if
      `key_fp` is None, zero open orders, `secret_gen` unchanged (bumped by `set_secret_ref`, so a
      stale run for an old secret cannot land), exactly 64 lowercase hex; caller check in the body.
      Check: tests incl. the stale-callback race.
- [x] M2.2b [must] Escrow `request_checkout(order_id)` → checkout mode → callback stores
      `authorization_url` (D-017). Check: tests + testnet run.
- [x] M2.2 (PR #12 review) refund the OutLayer fee when `claim` returns 0 (it slashed instead).
- [x] M2.2 (PR #11 review) the OutLayer callback `on_verdict_result(order_id, attempt)` (args built in the
      contract at claim time, never from the payload) reads the raw bytes with
      `promise_result_checked`, maps anything unparseable / unknown to `RunFailed` (never panics),
      then calls `apply_verdict`. Require `t_grace` > worst-case verification latency.
- [x] M2.2 Wire escrow `claim` to OutLayer `request_execution` + callback (from Test A). Check: testnet run.
- [ ] M2.3 End-to-end testnet buy: open → pay (Paystack test) → claim → USDC released. Check: script + tx links.
- [x] M2.4 Revoked-key demo: UNVERIFIABLE → wait `T_retry` → slash pays buyer. Check: script + tx links.
      (Done Nov 4 with a fake key on escrow-d1: scripts/buy.sh, tx links in PROGRESS.)
- [ ] M2.6 Reproducible verifier build (rust-toolchain.toml; OutLayer GitHub compile if possible,
      SPEC Q7). Check: a second build gives the same sha256.
- [ ] M2.8 [must] (testnet scripts done: deploy-testnet.sh, lp-setup.sh, buy.sh; key deletion and the
      final reproducible deploy still open) `scripts/deploy.sh` (mock-ft, storage registrations, mint, batched deploy + `new`,
      delete key) and `scripts/lp-setup.sh` (store secret `--via` escrow, register LP). Testnet
      deploys before M3 are throwaway; the final reproducible deploy comes after M3.1, because the
      verifier must already include pawaPay and cannot change later. Check: fresh run works.
- [ ] M2.9 [nice] GitHub Actions: `cargo test` + `cargo near build reproducible-wasm` (avoids the
      Docker Hub rate limit, gives judges CI evidence). Check: green run.
- [ ] M2.7 (PR #11 review) do not deploy any escrow build to testnet before M2.2: until then `claim` is
      free and nothing delivers verdicts except the timeout path.
- [ ] M2.7 [must] Deploy the escrow from a reproducible build, with deploy + `new` in ONE batched transaction
      signed by the contract account (T32); then remove its full-access key, or document upgrade
      trust (inv. 9).
- [ ] M2.5 Hostile review (PROMPTS.md §8) of contract + verifier; fix findings with tests.

## M3 Sell flow + settlement (Nov 8–10)
- [ ] M3.0 [must] `scripts/lp-service` (minimal script, not a product): receives the seller's number
      + salt, sends the pawaPay payout with the salt metadata, calls `claim_payout`. Check: run.
- [ ] M3.1 [must] Sell order: open sell orders count in the pawapay LP's `open_orders` (inv. 4,
      PR #9 review); NEP-145 storage pre-payment, JSON `open_sell` msg, pawapay LP only,
      recipient commitment per SPEC §5 / D-018 (inv. 13), payoutId used once,
      UUID payout ids, `confirm_received`, `claim_payout` with pawaPay status + recipient check,
      refund with outage extension. Payout check via `PaymentProvider`: pawaPay sandbox (D-011),
      `SimulatedPayout` fallback in a separate labelled build (D-010). Check: tests.
- [ ] M3.2 Testnet end-to-end sell with pawaPay sandbox payouts (fallback: simulated, labelled).
      Check: script + tx links.
- [ ] M3.4 [nice] Keeper: a ~30-line script that re-checks UNVERIFIED orders during an outage
      (T31). The outage demo can use manual re-checks instead. Check: run.
- [ ] M3.3 [nice] `settlement/`: withdraw released USDC to Base via 1Click (per D on SPEC Q4). Check: tx links.

## M4 Web demo (Nov 9–11)
- [ ] M4.1 `web/` scaffold with NEAR wallet login. Check: runs locally.
- [ ] M4.6 Demo script shows four outcomes live: PAID → release; NOT_PAID (wrong amount / wrong
      recipient); UNVERIFIABLE `LP_FAULT` → slash; provider down → window extended to
      `T_outage_max`, honest LP protected (D-012). Trigger for the outage case: sell flow → pawaPay
      sandbox number 233593456129 returns IN_RECONCILIATION (observed in Test B2); buy flow → TBD
      (likely the labelled simulated provider returning `server_error`). Check: recorded run.
- [ ] M4.2 Buyer screen: open order, pay via Paystack test checkout, claim, see status. Check: manual run, screenshots.
- [ ] M4.3 LP screen: register, bond, liquidity, rate, open orders. Check: manual run.
- [ ] M4.4 [nice] Seller screen (the CLI + lp-service script with tx links can show the sell flow). Check: manual run.
- [ ] M4.5 Host demo (keep up until Nov 25). Check: public URL works from a fresh browser.

## Stretch (only if M2 is done by Nov 9)
- [x] ~~S1 `agent-api/` x402 endpoint~~ DROPPED (D-010): needs real payouts.

## M5 Submission (Nov 12–14) — feature freeze Nov 12
- [ ] M5.1 (PR #12 review) README: a buyer-caused provider outage lasting the whole window can still
      slash an LP whose buyer never paid (residual risk, D-012/T31).
- [ ] M5.1 README for judges (PROMPTS.md §9): do X → see Y, mermaid diagram, test-mode disclosure,
      payouts on the pawaPay sandbox and why (D-010, D-011), a buyer whose MoMo charge is pending
      must claim within the grace window (otherwise anyone can expire the order), slash timing incl. the 72 h outage
      limit and its residual risk (D-012), production = licensed LPs + Act 1154,
      libraries/AI code used, licensing note (Act 1154), limitations.
- [ ] M5.2 Fresh clone → setup → demo from scratch; fix breakages.
- [ ] M5.3 Secret scan of git history (repo is already public, D-008: also scan now and before every
      merge); LICENSE check.
- [ ] M5.4 Demo video (2 min) + X post tagging @NEARProtocol #NEARHackathon [K posts].
- [ ] M5.5 Submit: name, track `realworld` (`agents` dropped, D-017), mainnet payout account, repo,
      post link [K approves].
