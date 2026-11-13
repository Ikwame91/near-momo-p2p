# OpenRamp: Project History

The living record of this project, built from the full git history (`git log --stat`, oldest
first) and the code as it stands. Where a commit's purpose is not clear from its message, diff or
the docs it touched, this file says so. Detailed session notes stay in `docs/PROGRESS.md`; the
reasoning behind each choice stays in `docs/DECISIONS.md` (D-numbers below point there).

---

## 1. Overview

**What it does.** OpenRamp is a peer-to-peer ramp between mobile money and USDC on NEAR. A buyer
pays cedis by mobile money to a liquidity provider (LP). A NEAR escrow contract releases the LP's
USDC to the buyer only when a program running in an OutLayer TEE (trusted execution environment)
has checked the payment directly with the payment provider using the LP's own API key. If the LP's
side can't be checked (for example, the LP revoked its key) and this lasts beyond a retry window,
the LP's bond is slashed to the buyer. There is no owner, admin or dispute path (D-003, D-014).
First market: Ghana. Demo rails: Paystack **test mode** (collections) only. No real money.

**Scope today (PLAN v0.2, Nov 4):** buy flow only (cedis → USDC). The sell flow (USDC → cedis,
pawaPay sandbox payouts), NEAR Intents settlement, the keeper and the agent API are cut or dropped;
their designs and sandbox proofs stay in the docs.

**Stack**
| Part | Tech |
|---|---|
| Escrow contract | Rust, near-sdk 5.30, `contracts/escrow` |
| Test token (mUSDC) | near-contract-standards 5.30 NEP-141, `contracts/mock-ft` (test only, D-013) |
| Verifier (TEE program) | Rust → `wasm32-wasip2`, serde_json, sha2 0.10, wasi-http-client 0.2, `verifier/` |
| TEE / oracle | OutLayer (`outlayer.testnet`), `request_execution` + callback |
| Payment provider | Paystack test mode API (`/transaction/initialize`, `/transaction/verify`) |
| Web page | Plain JavaScript, Vite 8, NEAR Wallet Selector 10.1.4 (Meteor, Intear), `web/` (D-024) |
| Tooling | Rust 1.97.0 pinned (`rust-toolchain.toml`), cargo-near 0.22.0, near-cli-rs, outlayer CLI |
| Network | NEAR testnet |

**How to run**
```bash
# contract tests (escrow + mock token)
cd contracts && cargo test
# verifier tests
cd verifier && cargo test
# build the contracts' wasm (testnet deploys so far used the non-reproducible build)
cd contracts/escrow && cargo near build non-reproducible-wasm   # same in contracts/mock-ft
# build the verifier wasm and print its sha256
cd verifier && sh build.sh
# web page (buyer): dev server, tests, static build
cd web && npm install && npm run dev      # npm test, npm run build
```
Testnet (needs `NEAR_ACCOUNT_ID`, `NEAR_PRIVATE_KEY`; see `.env.example`):
1. `VERIFIER_URL=<raw GitHub URL pinned to a commit> VERIFIER_HASH=<sha256> scripts/deploy-testnet.sh <tag>`
2. The LP stores its Paystack test key as an OutLayer secret bound to the verifier hash, readable
   only through the escrow (`--access allow-all --via escrow-<tag>.<parent>`).
3. `scripts/lp-setup.sh <tag> <lp_account> <secret_profile>` (mint, register, bond, liquidity,
   fingerprint).
4. `scripts/buy.sh <tag> buyer | open | checkout | claim | expire | withdraw | order | claimable`.

---

## 2. Current architecture

**Components**
- **Escrow** (`contracts/escrow/src`):
  - `lib.rs`: state, fixed `Params`, `new` (callable only by the contract itself);
  - `lp.rs`: LP registry, bond and liquidity, withdrawals, secret ref, pause;
  - `orders.rs`: buy orders, GHS rounding, references `opr-<deploy_tag>-<id>`, expiry, views;
  - `verdicts.rs`: verdict handling, failure classes, slash, claimable balances, payouts;
  - `outlayer.rs`: `claim`, `request_fingerprint`, `request_checkout` and their callbacks;
  - `events.rs`: NEP-297 events.
  - Tests sit next to the code in `*_tests.rs`.
- **Verifier** (`verifier/src`):
  - `lib.rs`: pure decision logic (`decide`, `checkout_result`, `key_fp`, `reference_is_valid`);
  - `main.rs`: reads the input JSON, takes the key from the OutLayer secret `PAYSTACK_SECRET_KEY`,
    calls Paystack over wasi-http and prints one JSON line.
  - Modes: `fingerprint`, `checkout`, `verify`.
- **Built verifier** (`wasm/openramp-verifier.wasm`): served to OutLayer from a raw GitHub URL
  pinned to a commit, checked by its sha256 (D-008).
- **Scripts:**
  - `scripts/deploy-testnet.sh`, `lp-setup.sh` and `buy.sh` for testnet;
  - `scripts/test-a` and `scripts/test-b` hold the feasibility tests (records, not product code).
- **Web page** (`web/`, buyer only so far):
  - `src/main.js`: the buyer flow (amount, pay, verify, outcome) and the orders list;
  - `src/style.css`: design tokens, cobalt and night (docs/DESIGN.md);
  - `src/band.js`: the stamped adinkra progress band;
  - `src/calls.js`: every contract call the page makes, as plain data (unchanged by the redesign);
  - `src/near.js`: reads over JSON-RPC, signing through the wallet selector;
  - `src/format.js`: money maths, flow labels and error sentences, in BigInt, with tests;
  - `src/dom.js`, `theme.js`, `toast.js`, `motion.js`: small UI helpers;
  - `scripts/check-calls.mjs`: sends the same calls with a test key.
- **Not built:** the LP screen (M4.3), hosting (M4.5), `settlement/` (cut), `agent-api/` (dropped, D-010).

```mermaid
flowchart LR
    Buyer["Buyer<br/>(web/ page with a NEAR wallet, or scripts/buy.sh)"]
    LP["LP account"]
    Escrow["escrow-&lt;tag&gt;.ikwame.testnet<br/>(contracts/escrow)"]
    Token["musdc-&lt;tag&gt;.ikwame.testnet<br/>(mock USDC, NEP-141)"]
    OL["outlayer.testnet<br/>request_execution"]
    TEE["OutLayer TEE worker<br/>runs openramp-verifier.wasm"]
    GH["raw.githubusercontent.com<br/>wasm/openramp-verifier.wasm<br/>(pinned commit + sha256)"]
    Secrets["OutLayer secrets<br/>LP's Paystack test key<br/>(bound to wasm hash, via escrow only)"]
    PS["Paystack API<br/>(test mode)"]

    LP -- "ft_transfer_call bond / liquidity" --> Token --> Escrow
    LP -. "stores key" .-> Secrets
    Buyer -- "open_buy_order / request_checkout / claim" --> Escrow
    Escrow -- "request_execution (fixed source, fee, limits, LP's secret_ref)" --> OL
    OL --> TEE
    GH -- "fetch + hash check" --> TEE
    Secrets -- "PAYSTACK_SECRET_KEY" --> TEE
    TEE -- "initialize / verify" --> PS
    TEE -- "JSON result" --> OL
    OL -- "callback on_verdict_result / on_checkout_result / on_fingerprint_result" --> Escrow
    Buyer -- "pays MoMo on Paystack checkout page" --> PS
    Escrow -- "withdraw_claimable → ft_transfer" --> Token
```

**Deployments (NEAR testnet, parent account `ikwame.testnet`)**
| Deploy | Escrow / token | Verifier hash | deploy_tag | Status |
|---|---|---|---|---|
| d1 | `escrow-d1` / `musdc-d1` | `6a9ff46b…926f` | `f51ae4a2` | Used for the slash, PAID (via the charge API) and NOT_PAID runs on Nov 4. Its checkout mode fails because Paystack rejects the old email (superseded by d2). |
| d2 | `escrow-d2` / `musdc-d2` | `ecc0515c…bf7b` | `ad88e13b` | Current demo deployment. Deployed Nov 4 (escrow deploy + `new` tx 87GcoZAa7FGSD1ST9vCcPt47FzmtyD5nMonMCXYWUk89). Nov 10: real key stored as `paystack-d2`, fingerprinted, and order 0 paid through the real Paystack checkout page → PAID → released (tx links in PROGRESS Session 5). |

Both escrows are built from the code merged in PR #14; they differ only in the verifier hash
fixed at init.

---

## 3. Timeline

Dates are commit dates (UTC). "PR #n" means `Ikwame91/near-momo-p2p#n`. Merge commits are listed
with their PR.

### Phase 1: Setup and planning (Nov 1)
- **2026-11-01, `ee31eb5`:** project created.
  - What: agent rules (AGENTS.md, CLAUDE.md), SPEC, THREAT_MODEL, PLAN, PROGRESS, DECISIONS,
    PROMPTS, `.env.example`, LICENSE (MIT), README.
  - Why: D-001 (build this ramp instead of FeeRail), D-005 (name), D-006 (agent workflow files).
- **2026-11-01, `98c5f51`:** plan approval and session 1 handoff recorded (PLAN, PROGRESS, SPEC).
- **2026-11-01, `ff5180a`:** secrets check recorded.
  - What: NEAR keys come in as env vars; the Paystack key is injected by the network proxy, not an
    env var (`.env.example`, PROGRESS).
- **2026-11-01, `3d70a8e`:** M0.2, toolchain installed (cargo-near, near-cli-rs, outlayer CLI).
  - Docs only (PLAN, PROGRESS). OutLayer's `install.sh` was blocked, so the CLI came from crates.io.

### Phase 2: Feasibility Test A, OutLayer (Nov 1)
- **2026-11-01, `5db4208`:** M0.3 partial.
  - What: OutLayer hello-world project (`scripts/test-a/hello/*`) builds.
  - Blocked on testnet funds.
- **2026-11-01, `d15462a`:** OutLayer on-chain round trip works; fetching the wasm is blocked by a
  FastFS testnet outage (PROGRESS).
- **2026-11-01, `2b53680`, `ca57f8b`:** hello.wasm committed to `wasm/` and run via a raw GitHub URL.
  - Why: FastFS was down, D-008.
  - Files: `wasm/hello.wasm`, `wasm/README.md`, DECISIONS.
- **2026-11-01, `f238c44`:** PR #1 merged.
- **2026-11-01, `4122a1c`:** M0.4, a testnet contract calls OutLayer and stores the callback result.
  - Files: `scripts/test-a/contract/*`, THREAT_MODEL.
- **2026-11-01, `14ff01b`, `cfe9ccd`:** Paystack verifier prototype (`scripts/test-a/paystack-verify/*`,
  `wasm/paystack-verify.wasm`, `verify-run.sh`) runs on OutLayer; UNVERIFIABLE case done.
- **2026-11-01, `8165cdf`:** M0.5 done.
  - What: the verifier returns PAID / NOT_PAID / UNVERIFIABLE from real Paystack test data.
  - Docs only.
- **2026-11-01, `2b0df3f`, `8b501bd`:** main merged into the branch; PR #2 merged.

### Phase 3: Security audit (Nov 1)
- **2026-11-01, `4b42309`:** Kwame added `claude/agents/security-auditor.md` (a reviewer agent
  definition).
  - The commit message says only "with auditing instructions"; no doc records why it was added then.
- **2026-11-01, `511f2f3`:** M0.10, security design update from an audit.
  - What: SPEC v0.2, THREAT_MODEL T23–T30, D-009.
  - Also moved the agent file to `.claude/agents/`.

### Phase 4: Payout feasibility and scope (Nov 1–3)
- **2026-11-01, `1217955`:** Test B (`scripts/test-b/run.py`).
  - Result: the MoMo charge works, but a Paystack Starter account cannot send transfers.
- **2026-11-01, `ea500c4`:** the buy flow becomes the main demo and sell-flow payouts are simulated
  (D-010); the agent API is dropped.
  - Files: AGENTS/CLAUDE, README, SPEC, PLAN.
- **2026-11-01, `a1bee6a`:** Test B script: only `transfer_unavailable` counts as "blocked" (exit 2);
  other transfer errors fail.
- **2026-11-02, `c759d98`:** PR #3 merged (commit date Nov 2).
- **2026-11-02, `8207f1e`:** decisions recorded.
  - What: pawaPay sandbox for sell payouts (D-011); slash timing by failure class (D-012); SPEC v0.3.
- **2026-11-03, `10ae29f`:** PR #4 merged.
- **2026-11-03, `48ddcbe`:** Test B2 (`scripts/test-b/pawapay_payouts.py`).
  - Result: pawaPay sandbox payouts work, and the status reply returns the recipient phone.
- **2026-11-03, `cf01093`:** PR #6 merged.

### Phase 5: Escrow contract, M1 (Nov 3–4)
- **2026-11-03, `9f45845`:** M1.1, escrow scaffold with fixed deploy config, plus the mock USDC token.
  - Files: `contracts/*`. Decision: D-013.
- **2026-11-03, `c3e90b5`:** M1.1 fix-up.
  - What: no owner (D-014); `new` callable only by the contract; OutLayer call limits; Rust pinned
    (D-015, `rust-toolchain.toml`).
- **2026-11-03, `864790e`:** the reproducible contract build is blocked by the Docker Hub rate limit
  (HTTP 429), to be retried before deploy.
- **2026-11-03, `5786280`:** PR #5 merged.
- **2026-11-03, `7928297`, `af6dbe0`:** M1.2, LP registration, bond and liquidity deposits, withdrawals
  (`lp.rs`, `lp_tests.rs`).
- **2026-11-03, `a66c6b1`:** PR #7 review fix-ups (D-016).
  - What: quote changes require 1 yocto; safer withdrawal gas; ceilings on settings; mint cap on
    the mock token.
- **2026-11-03, `231b3be`, `35154b8`:** main merged into the LP branch, keeping both PROGRESS entries;
  PR #7 merged.
- **2026-11-03, `89f770e`, `17f63ea`:** decisions before M1.4.
  - What: D-017 (pending = outage, one provider per LP, checkout through the escrow, 401-only
    LP fault, `agents` track dropped); D-018 (sell recipient commitment via pawaPay metadata,
    `scripts/test-b/pawapay_metadata_probe.py`).
  - PR #8 merged.
- **2026-11-03, `8ce2699`:** M1.3.
  - What: NEP-297 events (`events.rs`), provider limit, secret lock while orders are open, bond
    rule, pause/resume (`lp_m13_tests.rs`).
- **2026-11-03, `96f3acb`, `2398bb5`, `cfeb4d8`, `9644154`:** PR #9 review fix-ups, then PR #9 merged.
  - What: stricter unlock, profile charset, lowercase pots.
  - Docs: D-019 PR process.
  - Unclear: `2398bb5` and `cfeb4d8` have nearly the same message; the first touched only PLAN.md
    and the second the other docs. It looks like one change split over two commits.
- **2026-11-03, `42a0aca`:** M1.4, buy orders.
  - What: references, GHS rounding, limits, expiry, views (`orders.rs`, `orders_tests.rs`).
- **2026-11-04, `5200c72`, `8c29ec2`:** PR #10 review fix-ups (D-020), then PR #10 merged.
  - What: O(1) order indexes, new-buyer history per (LP, buyer), order storage charged up front.
- **2026-11-04, `889ec74`:** M1.5, verdict handling, release, release delay, claimable payouts
  (`verdicts.rs`, `verdicts_tests.rs`).
- **2026-11-04, `1ce140c`, `bc44db6`:** PR #11 review fix-ups (D-021), then PR #11 merged.
  - What: the verdict path schedules no transfer; no order can stay stuck in VERIFYING.
- **2026-11-04, `bc08be7`:** M1.6, slash from the bond, automatically when the failure window is met
  (`slash_tests.rs`).
- **2026-11-04, `f4ed7a2`, `dd230da`:** PR #12 review fix-ups (D-022), then PR #12 merged.
  - What: PENDING from a check claimed after the grace window is NOT_PAID; slash windows are
    measured fairly.

### Phase 6: Schedule compression (Nov 4)
- **2026-11-04, `9f82951`, `6e3bb27`:** PLAN v0.2, then PR #13 merged.
  - What: build done by Nov 6, submit Nov 11, buy flow only, list of cuts, D-022 kept.
  - Why: Kwame's timeline.

### Phase 7: OutLayer integration and first testnet runs (Nov 4)
- **2026-11-04, `1676129`:** M1.7, the books check (inv. 6) as one test across all buy flows
  (`books_tests.rs`).
- **2026-11-04, `ccd6332`:** Verifier v2 (`verifier/*`, `wasm/openramp-verifier.wasm`, D-023).
  - What: fingerprint / checkout / verify modes, PENDING, `key_fp`, strict reference format.
- **2026-11-04, `ef9d04d`:** escrow ↔ OutLayer wiring (`outlayer.rs`, `outlayer_tests.rs`).
  - What: payable `claim`, `request_fingerprint`, `request_checkout`; callbacks never panic on the
    payload.
- **2026-11-04, `8262b81`, `de4c9b4`:** testnet scripts.
  - `deploy-testnet.sh`: mock token, plus escrow deploy + `new` in one tx signed by the escrow (T32).
  - `lp-setup.sh` and `buy.sh`.
- **2026-11-04, `2f7a246`:** testnet deploy d1 and the on-chain slash demo recorded.
  - What: a fake key leads to two `auth_error` checks, then a slash paid to the buyer.
  - M2.4 ticked; tx links in PROGRESS.
- **2026-11-04, `4d6fef2`:** checkout email changed from `buyer@openramp.example` to
  `buyer@example.com`, with a test; `buy.sh expire` step added.
  - Why: Paystack returned 400 `invalid_email_address`, so checkout failed on d1.
- **2026-11-04, `6d209e4`:** PAID and NOT_PAID runs with Kwame's real Paystack test key recorded.
  - PAID: the payment was made with the charge API, not the checkout page.
  - NOT_PAID: the order expired with no slash.
  - M2.3 ticked.
- **2026-11-04, `e64d8ac`:** PR #14 merged (all of Phase 7).

### Phase 8: d2 deploy for the checkout-page demo (Nov 4–10)
- **2026-11-04, `14fd1f8`:** verifier wasm rebuilt with the new email (sha256 `ecc0515c…`).
  - Then deployed as d2 (see §2; the deploy is on-chain, not a commit).
- **2026-11-04, `27de834`:** README note on why Paystack needs a buyer email and how a real buyer
  would provide it; `lp-setup.sh` gets `SKIP_FINGERPRINT=1`.
  - Why: Kwame asked for the README note; the flag lets LP setup run before the secret exists.
- **2026-11-05, `c08e6fd`:** `docs/PROJECT_HISTORY.md` created, and the "Project history (required)"
  rule added to CLAUDE.md and AGENTS.md.
- **2026-11-10, `0142d93`:** PAID through the real Paystack checkout page on d2 recorded
  (PROGRESS Session 5, PLAN M2.3b).
  - Steps: real key stored as `paystack-d2`; fingerprint `9d2894e2…`; checkout URL stored on-chain;
    Kwame paid with test MoMo; claim → `order_released`; buyer withdrew 2 mUSDC.
  - Files: docs only.
- **2026-11-10, `cfac207`:** PR #15 merged (all of Phase 8).

### Phase 9: Web page (Nov 10–11; branch `web/buyer-page`)
- **2026-11-10, `ac69961`:** buyer page in `web/` (M4.1 done; M4.2 built).
  - What: live LP quote, open order, Paystack link, check payment, close unpaid order, withdraw;
    read-only `?view=<account>` mode; light and dark.
  - Why: until now buyers could only use `scripts/buy.sh`; the demo needs a page.
  - Checked: loads against escrow-d2 in a browser; every call it makes was sent with a throwaway
    test key and accepted by the escrow (tx links in PROGRESS Session 5b).
  - Files: `web/*`, `docs/screenshots/buyer-page.png`, README, DECISIONS (D-024), PLAN, PROGRESS.
- **2026-11-10, `eb0674b`:** fix: the page stayed blank under `npm run dev`.
  - Why: `@near-js/accounts` reads `process.env` when it loads; the production build replaces it,
    the dev server does not (`ReferenceError: process is not defined`). Kwame hit it on his first run;
    my earlier checks had only used the production build.
  - Fix: `web/src/buffer-shim.js` also provides an empty `process.env`. Checked both
    `npm run dev` and the built page in a browser.
- **2026-11-11, `0fcfee0`:** buyer page redesigned as a guided fintech flow (D-025). Made with Cursor.
  - What: four-step tracker, live USDC/GHS amount, Paystack countdown from `t_pay_ns`, enclave
    verify state, separate success / expired / failed / slashed cards, toasts, orders list, light
    and dark. The seller account is refused before the wallet opens.
  - Why: the first page worked but looked unfinished, and Meteor was showing a raw contract panic
    when `ikwame.testnet` tried to buy from itself.
  - Unchanged: `web/src/calls.js` and `web/src/near.js` (same calls, deposits, gas, RPC, wallet setup).
  - Checked: `npm test` (10) and `npm run build`. Bundle grew about 20 KB. Screenshots in
    `docs/screenshots/buyer-redesign/`.
  - Still open: the happy path needs a buyer account that is not the seller (M4.2).
- **2026-11-11, `d332367`:** five fixes from reviewing the redesign (authored as Kwame at his request).
  - Expired orders are neutral grey, not error red (pill and card); the expired card's tracker
    shows where the order stopped with no step marked current, and its text says whether the
    seller's USDC is already unlocked.
  - The header stays on one row (it wrapped under the logo, even on desktop).
  - A cancelled or failed "open order" stays on the amount screen; before, the page jumped to the
    newest old order.
  - Each order's explorer link points at that order's own last transaction from this browser
    (kept per contract in local storage); before, every card linked the last transaction sent.
  - The orders list follows the clock: "time ago" updates, and an unpaid order reads Expired once
    its pay window and grace have passed, without waiting for other data to change.
  - Files: `web/src/main.js`, `format.js` (+ test), `style.css`, `dom.js`, after-screenshots.
  - Checked: `npm test` (10), build, browser run with no console errors. Not changed: `calls.js`, `near.js`.
- **2026-11-11, OutLayer testnet outage (no commit).** From about 21:50 GMT OutLayer accepted
  requests but its worker never ran them ("Execution promise failed … Refunded" after ~2 min);
  Kwame confirmed other projects failed too. firmstreet.testnet's order 4 got two "couldn't
  verify" failures (21:55, 21:57) and is left alone. A probe (hello-world run, no order touched)
  checks OutLayer every 30 min until midnight GMT and again from 07:00.
- **2026-11-11, `9b2eb39`:** the page says "Checking with the secure enclave… this can take up to
  2 minutes" while OutLayer works, explains an empty answer ("The verification service didn't
  answer. Your money is safe. Try again in a few minutes."), and asks for 1 testnet NEAR (0.5 was
  too little: a 300 TGas call needs ~0.31 NEAR free). Files: `web/src/main.js`, `format.js` (+ test).
- **2026-11-11, `ac619f3`:** read-only seller page (`?seller`), M4.3 (D-026). Files: `web/src/seller.js`,
  `main.js`, `format.js` (+ test), `style.css`, `docs/screenshots/seller-page.png`.
- **2026-11-11, `0adf503`:** README rewritten for judges (how it works, testnet proof, how to try,
  honest limits, upgrade-key plan); `wasm/README.md` hash fixed to `ecc0515c…`.
- **2026-11-11, `e2b3a2c`:** `web/vercel.json` and `docs/DEPLOY_WEB.md` (not deployed).
- **2026-11-11, `1429c06`:** `docs/DEMO_SCRIPT.md`, live plan and a no-OutLayer fallback.
- **2026-11-11, `bc6d0c8`:** PROGRESS, PROJECT_HISTORY and DECISIONS (D-026) for the above.
- **2026-11-11, `a2f6033`:** commit IDs above updated after re-authoring the six commits as Kwame (his request; content unchanged).
- **2026-11-11, `36ae36e`:** M1.9 immutability tests (`contracts/escrow/src/immutability_tests.rs`): the public
  method list is pinned and has no settings setter; no code writes token / OutLayer / verifier /
  params / deploy tag after `new`; the settings are identical after every kind of flow.
  Mutation-checked (a planted nested write and a planted `self.token =` both fail the tests).
  Also README: the disclosures the hackathon rules require (work built in the window, libraries,
  AI-generated code including commits re-authored as Kwame) and Rust/Node prerequisites.
- **2026-11-11, `3c34213`:** GitHub Actions workflow `.github/workflows/tests.yml` (contracts, verifier, web).
  Not yet seen running: it needs a push.
- **2026-11-11, `1dc45a8`:** PROGRESS / PROJECT_HISTORY for the above.
- **2026-11-12, `4af06bc`:** README states this is Kwame's only hackathon submission. Pushed with the
  three commits above, as Claude (Kwame's choice: no more re-authoring).
- **2026-11-12, CI run #1 green** on `4af06bc`: contracts 267 + 5, verifier 20, web 12 + build
  (https://github.com/Ikwame91/near-momo-p2p/actions/runs/43277945976).
- **2026-11-12, `c9d3317`:** README drops "No CI" and shows the CI badge (CI green again).
- **2026-11-12, M4.2 done (no code change).** Kwame bought 1 USDC through the web page with Meteor as
  firmstreet.testnet: d2 order 6, real Paystack checkout, paid, RELEASED, withdrawn (PROGRESS Nov 12).
- **2026-11-12, `4be7942`:** PLAN / PROGRESS / PROJECT_HISTORY / README record it.
- **2026-11-12, `e5c9098`:** PR #16 merged: web page, judge README, M1.9 tests and CI are on `main`.
- **2026-11-12, Vercel (no commit):** Kwame deployed `web/` from `main`: https://near-momo-p2p.vercel.app.
- **2026-11-12, `fd2800c` (merged as `cfa2aee`):** README, PLAN (M4.5), DEPLOY_WEB, PROGRESS link the live site.
- **2026-11-13, OutLayer outage (no commit):** OutLayer testnet failed every probe from about 07:00 to 16:04 UTC, then
  answered at 16:34. d2 order 7 was then closed with one claim (NOT_PAID, `not_found`, so EXPIRED; bond untouched).
  Kwame bought 2 USDC live through the page as d2 order 8 (RELEASED).
- **2026-11-13, `4f6defa` (merged as `e11b056`, PR #18):** demo polish from the video review. Only the pressed button shows the spinner
  (`actionButton` in `web/src/main.js`), one toast at a time (`web/src/toast.js`), the wallet modal follows the
  page's dark theme (`web/src/near.js`), description and Open Graph tags (`web/index.html`), and the README
  opening no longer says "no owner, no admin" while escrow-d2 keeps its full-access key.
- **2026-11-13, `1bfb75a`:** web redesign, "wax print" (D-027, `docs/DESIGN.md`). Three directions were
  rendered with the App Designer process; Kwame picked the wax print with the sealed headline. Cobalt colour
  field, tomato primary action, Bricolage Grotesque, a stamped adinkra band (`web/src/band.js`) instead of the
  step tracker, "per USDC" and "the seller (ikwame.testnet)" wording, numbered "How it works". UI layer only.
- **2026-11-13, `491e6cd` (merged as `3fdaac3`, PR #19):** redesign polish from Grok's review: one lighter panel for the current step, order
  lists show the newest 3 with "See all", the read-only notice is a small pill, "Close order" only when it can be
  used, the seller page leads with the guarantee and the amount available, the received amount counts up with a
  stamped tick and a "View transaction" button, and "How it works" step 1 no longer says "no admin functions".
- **2026-11-13, on-chain (no commit):** orders 3, 5, 9 and 10 closed with `expire_order`; OutLayer down again from
  about 20:10 UTC (order 11 opened, no checkout).
- **2026-11-13, this commit:** UI review with Vercel's `web-design-guidelines` skill (D-028, `docs/UI_REVIEW.md`),
  skill committed under `.claude/skills/`. Specific status labels, a bond-window countdown (`bondPaysAt`, mirrors
  `due_at`), NEAR fees before confirming (`fmtNear`), Copy buttons, hover/tap/skip-link/theme-color fixes.

---

## 4. Decisions log

Short form. Full context in `docs/DECISIONS.md`.

| # | Choice | Why | Alternatives rejected |
|---|---|---|---|
| D-001 | Build a trustless P2P MoMo ⇄ USDC ramp | Cash-out is the main crypto pain in Africa; fits Real-World Money | FeeRail, MoMo-for-agents, AI Bursar |
| D-002 | Paystack test mode only, provider as a plug-in | Paystack ToS bans crypto/escrow in live mode | Live Paystack, other providers |
| D-003 | Bond slashing, no disputes | An arbiter breaks trustlessness; key revocation needs a remedy | Admin arbiter; UNVERIFIABLE = NOT_PAID |
| D-004 | Mainnet dev account (≤ $1) for NEAR Intents | Intents has no testnet (now cut) | Mock settlement only |
| D-005 | Name "OpenRamp" | Not tied to Ghana | SikaSwap, Rampart, PayProof, Cashbridge |
| D-006 | Rules in AGENTS.md (CLAUDE.md a copy), state in docs/ | Agent-driven workflow | — |
| D-007 | Escrow network for the demo | **Still open** (SPEC Q4, M0.9) | — |
| D-008 | Host wasm in this repo, raw GitHub URL pinned to a commit + sha256 | FastFS testnet was down; OutLayer accepts any https URL | FastFS, separate repo |
| D-009 | Security update: key fingerprint, contract-set OutLayer limits, every failure stays UNVERIFIED, fixed verifier/token, references `opr-<tag>-<id>` | Audit found ways to take funds | On-chain recipient number; Paystack integration id |
| D-010 | Buy flow is the main demo; sell payout simulated; agent API dropped | Paystack Starter cannot send transfers | Upgrade Paystack; new payout provider |
| D-011 | pawaPay sandbox for sell payouts (demo only) | Instant sandbox, returns recipient phone | Korapay (fallback) |
| D-012 | Failure classes: LP_FAULT → `t_retry`, OUTAGE → `t_outage_max`; every failure ends in a slash if it never clears | Fair to honest LPs; faking an outage gains nothing | No slash during outages; one window for all |
| D-013 | near-contract-standards for mock USDC | Less code to get wrong | Hand-written NEP-141; real testnet USDC |
| D-014 | No owner; all params fixed at init | "Nobody can change the rules"; one less key to steal | Owner until the first order |
| D-015 | Rust 1.97.0 pinned; reproducible contract builds for any hash that matters | Judges must be able to check deployed code | README-only pin; skip reproducibility |
| D-016 | Withdrawal gas weighting, 1-yocto LP settings, setting ceilings, `min_failed_attempts` 2–10, mint cap | Review findings, each on its own merits | Gas setter (needs an owner); unbounded settings |
| D-017 | Pending = outage; one provider per LP; checkout through the escrow (GHS, mobile_money, synthetic email); only 401 = LP fault; `agents` track dropped | Review before M1.4 | — |
| D-018 | Sell recipient commitment via salted hash + pawaPay metadata | Unsalted phone hashes are crackable | Salt as OutLayer input; per-order secret |
| D-019 | Claude opens and merges PRs after tests; Kwame reviews after. Subagent reviews paused Nov 4 | Speed; save credits | — |
| D-020 | New-buyer history per (LP, buyer); storage charged up front; O(1) indexes | Prevent cap bypass, storage drain, gas growth | Global counter; refund storage on close |
| D-021 | Verdict path only credits a claimable balance; 15-min VERIFY_TIMEOUT | Payout inside the callback ran out of gas and froze orders | Higher callback gas; RELEASE_PENDING state |
| D-022 | PENDING after `pay_deadline + t_grace` is NOT_PAID; fair slash-window measurement | Stops a "pending forever" attack on LPs | Cap PENDING only in the verifier |
| D-023 | `sha2` for `key_fp` in the verifier | Don't hand-write SHA-256 | Hand-written SHA-256 |
| D-024 | Web page in plain JS + Vite + NEAR Wallet Selector (Meteor, Intear); no MyNearWallet; calls kept in one file and checked with a test key | One page doesn't need a framework; MyNearWallet closed app connections on Nov 1 | React starter; redirect-wallet flow; key in the browser |
| D-025 | Buyer page redesign stays in the UI layer: CSS motion, no new packages | Keep the same contract calls and stay inside a ~100 KB bundle budget | `motion` npm package; editing `calls.js` or the wallet setup |
| D-028 | Commit Vercel's `web-design-guidelines` skill for UI reviews; no React rewrite | Page felt unfinished; web/ is plain JS | shadcn/ui + React rewrite; frontend-design |
| D-027 | "Wax print" redesign: cobalt field, one tomato action, Bricolage Grotesque (Google Fonts), stamped adinkra progress band | The page read as a generic green fintech form | Forex rate board (dark finance cliche); sealed envelope (printed form) |
| D-026 | Read-only seller page; delete the escrow's full-access key on the final deploy; no checks during an OutLayer outage | No new calls for the LP page; "nobody can change the rules" must hold; a failed check counts toward a slash | LP actions in the page; keep the key and state the trust |

**Choices made in commits but not (yet) in DECISIONS.md**
- **Escrow deploy + `new` in one transaction signed by the escrow account** (`8262b81`). From
  THREAT_MODEL T32: nobody else can initialise it with other params.
- **Checkout email `buyer@example.com`** (`4d6fef2`). Paystack rejects `.example`; `example.com`
  is a reserved domain with no inbox. The alternative for real buyers (a per-order alias) is
  described in the README and not built.
- **LP secrets bound to the verifier hash and readable only via the escrow**
  (`--access allow-all --via escrow-<tag>`; README / scripts). So the key can only be used for
  calls the escrow makes.
- **Testnet escrows built with `cargo near build non-reproducible-wasm`.** This is a shortcut
  against D-015, taken because of the Docker Hub 429 (`864790e`); see Known issues.

---

## 5. Known issues and tech debt

**Open work for the current schedule**
- The payment step can't be automated: Paystack's checkout page sits behind a Cloudflare bot check,
  so a person has to pay (fine for the demo, but it means no automated end-to-end test).
- **M4.2 done (Nov 12):** a person bought USDC through the page with Meteor (d2 order 6).
- The web page shows one seller only (`VITE_DEFAULT_LP`): the escrow has no "list LPs" view.
- **Depends on OutLayer testnet uptime.** It was down Nov 11 ~21:50–23:00 GMT; the page explains an empty answer, and the demo script has a fallback.
- **Hosted:** https://near-momo-p2p.vercel.app (Vercel Hobby, from `main`). Keep it up through Nov 25.
- `ikwame.testnet` is down to about 0.17 NEAR; it needs a faucet top-up before another deploy.
  Test accounts were funded from `escrow-d1` (superseded deploy) and `buyer-d2`.
- `npm install` in `web/` reports audit warnings from the wallet libraries' dependencies; not reviewed.
- Test-day tasks still open: M1.8 near-workspaces sandbox tests (including the gas of the
  slash-in-verdict path), M1.9 immutability tests, M5.2 fresh-clone run, M2.5 hostile review.

**Shortcuts and risks**
- **Builds are not reproducible.**
  - Testnet escrows come from non-reproducible builds, against D-015; the verifier wasm is a local
    build too (M2.6).
  - Nobody else can yet rebuild them to the same hash.
- **Escrow full-access keys are not removed** after deploy (M2.7). The testnet escrows are
  upgradeable by whoever holds the key. The README says so; the plan is to delete the key on the
  final deploy (D-026).
- **CI runs tests only.** `.github/workflows/tests.yml` runs contracts, verifier and web tests and the
  web build on every push (first green run: https://github.com/Ikwame91/near-momo-p2p/actions/runs/43277945976).
  It does not run clippy or reproducible builds.
- **Provider plug-in is only partly there.** D-002 and AGENTS.md describe a `PaymentProvider`
  interface, but the verifier is Paystack-only with no such trait in the code.
- **Real-buyer email** (per-order alias) is design only.
- **Trust in OutLayer.** The escrow takes the TEE's output from OutLayer's callback, with no
  separate signature to check (as AGENTS.md anticipated). It does check that the verdict carries
  this order's reference and the LP's registered `key_fp` (`verdicts.rs`).
- **Deprecated test helper.** `outlayer_tests.rs` uses deprecated `testing_env_with_promise_results`
  under `#![allow(deprecated)]`; the replacement needs an extra dev-dependency.
- **`deploy-testnet.sh` and `buy.sh buyer` read the public key from `outlayer whoami`**, so they
  only work where the outlayer CLI is logged in as the parent account.

**Web page**
- A theme flip to light after withdraw was seen in the demo video but not reproduced: the page kept its
  theme through refreshes in dark, saved-dark and saved-light setups. Likely the wallet's own light approval
  window. A light page on a dark OS still gets a dark wallet modal (the library has no light override).

**Docs out of date**
- DECISIONS D-022 still says "Kwame to confirm"; PLAN v0.2 records that it stays.
- D-019 says the AGENTS.md merge rule should be updated; it hasn't been.
- AGENTS.md / CLAUDE.md still describe `settlement/`, `web/` and the sell flow as planned modules;
  PLAN v0.2 cuts most of them. SPEC v0.4 still describes the sell flow in full.
- D-007 (escrow network) is still open, and M0.9 is unticked.

**Cut or dropped (kept as designs or records)**
- Sell flow (M3), NEAR Intents settlement (Test C, M3.3), keeper (M3.4), M1.10, M2.9, M4.4, and the
  agent API (D-010).
- `scripts/test-a`, `scripts/test-b`, `wasm/hello.wasm` and `wasm/paystack-verify.wasm` are
  feasibility records, not product code.
- d1 is superseded by d2 but still deployed, with an old verifier whose checkout mode fails.

---

## 6. Last updated

2026-11-13. Covers all commits up to and including `3fdaac3` (`main`), plus the UI review commit on
`claude/practical-dijkstra-gt0v80`.
