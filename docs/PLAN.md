# OpenRamp: Plan

Status: v0.1 APPROVED by Kwame (Nov 1, 2026). Each task ≤ 2 h and has a check.
`[K]` = Kwame must do it by hand (accounts, keys, funding). Feature freeze: Nov 12.

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
- [ ] M1.2 LP registration, bond and liquidity deposits via `ft_transfer_call`, withdrawals. Check: tests.
- [ ] M1.3 Bond/secret lock while orders open (inv. 4), bond rule (inv. 5), `pause_lp` (inv. 14). Check: failing→passing tests.
- [ ] M1.4 Buy order open/expire, `opr-<deploy_tag>-<id>` references (inv. 12), new-buyer cap + per-LP
      exposure cap (inv. 14). Check: tests.
- [ ] M1.5 Verdict handling behind a `Verifier` trait with a mock: PAID/NOT_PAID/UNVERIFIABLE paths,
      replay protection, reference + `key_fp` must match (inv. 1–3, 10). Check: tests.
- [ ] M1.6 Retry window + slash with `min_failed_attempts`, failure classes `LP_FAULT` (`T_retry`) /
      `OUTAGE` (`T_outage_max`), contract-set OutLayer deposit/limits (T5, T8, T25, T30, T31,
      inv. 11, D-012). Check: tests with time travel.
- [ ] M1.7 Balance accounting invariant (inv. 6) checked after every test. Check: tests.
- [ ] M1.9 Immutability: no setter for verifier/token/OutLayer id; `ft_on_transfer` rejects other
      tokens (inv. 9, T26). Check: tests.
- [ ] M1.8 Sandbox integration test (near-workspaces) of a full buy flow with mock verifier. Check: passes.

## M2 Verifier + real wiring (Nov 6–8)
- [ ] M2.1 `verifier/` crate (start from `scripts/test-a/paystack-verify`, which already has the
      verdict logic + 15 tests): `PaymentProvider` trait, Paystack adapter, verdict mapping, unit tests
      with recorded test-mode responses. Check: tests.
- [ ] M2.1b Verifier modes: `fingerprint` (returns `key_fp`), `checkout` (creates the Paystack
      checkout with the LP key, SPEC Q6), `verify` output carries `key_fp`. Check: tests + testnet run.
- [ ] M2.2 Wire escrow `claim` to OutLayer `request_execution` + callback (from Test A). Check: testnet run.
- [ ] M2.3 End-to-end testnet buy: open → pay (Paystack test) → claim → USDC released. Check: script + tx links.
- [ ] M2.4 Revoked-key demo: UNVERIFIABLE → wait `T_retry` → slash pays buyer. Check: script + tx links.
- [ ] M2.6 Reproducible verifier build (rust-toolchain.toml; OutLayer GitHub compile if possible,
      SPEC Q7). Check: a second build gives the same sha256.
- [ ] M2.7 Deploy the escrow from a reproducible build, with deploy + `new` in ONE batched transaction
      signed by the contract account (T32); then remove its full-access key, or document upgrade
      trust (inv. 9).
- [ ] M2.5 Hostile review (PROMPTS.md §8) of contract + verifier; fix findings with tests.

## M3 Sell flow + settlement (Nov 8–10)
- [ ] M3.1 Sell order: lock, recipient commitment with canonical `233…` numbers (inv. 13, SPEC Q5),
      UUID payout ids, `confirm_received`, `claim_payout` with pawaPay status + recipient check,
      refund with outage extension. Payout check via `PaymentProvider`: pawaPay sandbox (D-011),
      `SimulatedPayout` fallback in a separate labelled build (D-010). Check: tests.
- [ ] M3.2 Testnet end-to-end sell with pawaPay sandbox payouts (fallback: simulated, labelled).
      Check: script + tx links.
- [ ] M3.3 `settlement/`: withdraw released USDC to Base via 1Click (per D on SPEC Q4). Check: tx links.

## M4 Web demo (Nov 9–11)
- [ ] M4.1 `web/` scaffold with NEAR wallet login. Check: runs locally.
- [ ] M4.6 Demo script shows four outcomes live: PAID → release; NOT_PAID (wrong amount / wrong
      recipient); UNVERIFIABLE `LP_FAULT` → slash; provider down → window extended to
      `T_outage_max`, honest LP protected (D-012). Trigger for the outage case: sell flow → pawaPay
      sandbox number 233593456129 returns IN_RECONCILIATION (observed in Test B2); buy flow → TBD
      (likely the labelled simulated provider returning `server_error`). Check: recorded run.
- [ ] M4.2 Buyer screen: open order, pay via Paystack test checkout, claim, see status. Check: manual run, screenshots.
- [ ] M4.3 LP screen: register, bond, liquidity, rate, open orders. Check: manual run.
- [ ] M4.4 Seller screen. Check: manual run.
- [ ] M4.5 Host demo (keep up until Nov 25). Check: public URL works from a fresh browser.

## Stretch (only if M2 is done by Nov 9)
- [x] ~~S1 `agent-api/` x402 endpoint~~ DROPPED (D-010): needs real payouts.

## M5 Submission (Nov 12–14) — feature freeze Nov 12
- [ ] M5.1 README for judges (PROMPTS.md §9): do X → see Y, mermaid diagram, test-mode disclosure,
      payouts on the pawaPay sandbox and why (D-010, D-011), slash timing incl. the 72 h outage
      limit and its residual risk (D-012), production = licensed LPs + Act 1154,
      libraries/AI code used, licensing note (Act 1154), limitations.
- [ ] M5.2 Fresh clone → setup → demo from scratch; fix breakages.
- [ ] M5.3 Secret scan of git history; add LICENSE check; make repo public [K approves].
- [ ] M5.4 Demo video (2 min) + X post tagging @NEARProtocol #NEARHackathon [K posts].
- [ ] M5.5 Submit: name, tracks (`realworld`, `agents`), mainnet payout account, repo, post link [K approves].
