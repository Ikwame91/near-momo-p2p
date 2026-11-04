# Decisions

Template:
```
## D-NNN Title (date)
Status: accepted | superseded by D-xxx | open
Context: why we had to decide
Decision: what we chose
Alternatives: what we rejected and why
```

## D-001 Trustless P2P mobile money ⇄ USDC ramp instead of FeeRail (Oct 2026)
Status: accepted
Context: FeeRail (diaspora school fees) scored 21/25 but had low originality and no live schools.
Decision: Build a P2P on/off-ramp where an OutLayer TEE verifies mobile money payments and a NEAR
escrow releases USDC. Cash-out is the main crypto pain in Africa; fits Real-World Money and Agent Economy.
Alternatives: FeeRail; MoMo-for-agents via x402 (kept as stretch S1, later dropped in D-010); AI Bursar.

## D-002 Paystack test mode behind a provider interface (Oct 2026)
Status: accepted
Context: Paystack's terms ban crypto, FX and escrow businesses in live mode.
Decision: Demo in Paystack test mode only; verifier uses a `PaymentProvider` trait so a licensed
partner can replace Paystack. README states this plainly.
Alternatives: Live Paystack (breaks ToS); other providers (not validated in time).

## D-003 Bond slashing instead of disputes (Oct 2026)
Status: accepted
Context: An LP could revoke their key after receiving cedis; an arbiter would break trustlessness.
Decision: LPs post a USDC bond ≥ open buy notional. UNVERIFIABLE for longer than `T_retry` slashes
the bond to the buyer. No admin, no dispute path.
Alternatives: Admin arbiter (centralised); treat UNVERIFIABLE as NOT_PAID (lets LPs steal).

## D-004 Mainnet dev account for NEAR Intents (Nov 1, 2026)
Status: accepted
Context: NEAR Intents has no testnet (hackathon rules confirm this).
Decision: Test C and settlement use a dedicated mainnet dev account, ≤ $1 per transfer, dry-run
quotes first. Approved by Kwame as part of the Test C plan.
Alternatives: Mock settlement only (weaker demo).

## D-005 Project name OpenRamp (Nov 1, 2026)
Status: accepted
Context: Global competition; name should not be limited to Ghana.
Decision: OpenRamp — an open, trustless on/off-ramp. Ghana is the first market in the demo.
Alternatives: SikaSwap (too local), Rampart, PayProof, Cashbridge.

## D-006 Agent workflow files (Nov 1, 2026)
Status: accepted
Decision: Rules in AGENTS.md (CLAUDE.md is an exact copy), state in docs/SPEC, PLAN, PROGRESS,
DECISIONS, THREAT_MODEL; prompts in docs/PROMPTS.md. Push freely; ask before going public/submitting.

## D-007 Escrow network for the demo (open)
Status: open — decide in M0.9 (SPEC Q4)

## D-008 Host OutLayer wasm files in this repo's `wasm/` folder (Nov 1, 2026)
Status: accepted
Context: FastFS testnet (test.fastfs.io) returned 502 for everyone, so OutLayer workers could not
fetch our program. OutLayer docs: "Any https URL works"; the worker checks the file's sha256.
The repo is already public.
Decision: commit built `.wasm` files to `wasm/` and give OutLayer the raw GitHub URL pinned to a
commit SHA (`https://raw.githubusercontent.com/Ikwame91/near-momo-p2p/<sha>/wasm/<file>`), with
`--hash`. Never put secrets there; secrets reach programs only as OutLayer secrets.
Alternatives: FastFS (preferred when it is up; recheck each session), a separate public repo
(not needed since this repo is public).

## D-009 Security design update from the audit (Nov 1, 2026)
Status: accepted (Kwame, "go")
Context: A design audit of SPEC v0.1 found ways for an LP or buyer to take funds that the SPEC
did not cover. Test A results (M0.4–M0.5) answered two of its findings (secret access, callback).
Decision: SPEC v0.2 adds invariants 9–14 and THREAT_MODEL T23–T30:
- Sell: seller commits to the payout recipient (salted hash); paying anyone else is NOT_PAID.
- Buy: the TEE records the LP key fingerprint at registration; verdicts carry it; mismatch is
  UNVERIFIABLE. The checkout is created in the TEE with the same key.
- The contract sets OutLayer deposit/gas/limits. EVERY failure stays UNVERIFIED (we rejected the
  audit's "buyer-caused errors don't count": the contract only sees `null` and an LP can cause
  failures too by deleting or locking its secret). Slash needs `min_failed_attempts` spanning T_retry.
- Verifier source, token and OutLayer id fixed at init; no full-access key (or stated upgrade trust).
- References `opr-<deploy_tag>-<order_id>`; LP pause; per-LP new-buyer exposure cap.
Alternatives: on-chain recipient number (rejected: privacy); Paystack integration id instead of
key fingerprint (rejected: not in the verify response, checked 2026-11-01).

## D-010 Buy flow is the main demo; sell-flow payout check is simulated (Nov 1, 2026)
Status: accepted (Kwame, option 3); payouts UPDATED by D-011 (pawaPay sandbox)
Context: Test B (M0.6) showed a Paystack Starter business cannot send transfers, even in test
mode ("You cannot initiate third party payouts as a starter business"); a Registered Business
upgrade needs business papers and may take days. The buy flow only receives payments, which work.
Decision:
- The buy flow (cedis → USDC) is the main, fully real demo (Paystack test mode + OutLayer TEE).
- The sell flow is still built and tested in the contract (lock, recipient commitment,
  confirm_received, claim_payout, refund). Only the "did the payout land" check is simulated, by a
  `SimulatedPayout` provider behind the `PaymentProvider` interface.
- The simulated provider is a SEPARATE verifier build with its own wasm hash, used only by a
  demo deployment that the README and UI label "simulated payout". It is never part of the
  buy-flow verifier and never used with real money (it would be a release backdoor).
- The README states plainly that real payouts need a Registered Business on Paystack or a
  licensed payout partner.
- S1 (`agent-api/` x402: an agent pays a Ghana number in cedis) is dropped: it needs real payouts.
Alternatives: upgrade Paystack (kept as an optional side track; if approved in time, switch
the sell flow to real transfers); another payout provider (rejected: new research, 13 days left).

## D-011 pawaPay sandbox for sell-flow payouts (demo only); production needs licensed LPs (Nov 2, 2026)
Status: accepted (Kwame). Updates D-010.
Context: Paystack Starter cannot send transfers (Test B). pawaPay (suggested by Jeffery, who is
not on the project) gives an instant sandbox with no onboarding ("You do not need to go through
the onboarding process for that since no real money is involved",
https://docs.pawapay.io/v2/docs/how_to_start), Ghana test numbers for MTN, AirtelTigo and
Vodafone/Telecel, and a payout status that returns the recipient's phone number and provider,
which the T23 recipient check needs.
Decision:
- Buy flow stays on Paystack test mode (proven in M0.5).
- Sell-flow payouts use the pawaPay sandbox behind the `PaymentProvider` interface; the
  `SimulatedPayout` provider (D-010) stays as a labelled fallback.
- pawaPay is a demo sandbox, NOT the production rail: its prohibited list bans "unlicensed …
  money transfer" (https://www.pawapay.io/prohibited-business-activity), live access needs a legal
  business, licences and compliance approval, and Act 1154 requires a VASP licence. In production,
  LPs are licensed businesses or partners (e.g. Kotani Pay, Fonbnk) and OpenRamp is the trust
  layer between them and users; buyers still never trust the LP.
- Fallback: if pawaPay will not enable Ghana or fund the sandbox wallet within a day, use a
  Korapay "Individual" test account.
- Not now: pawaPay deposits for the buy flow (one account per LP, phone-prompt UX). Revisit only
  after M2 if there is time.
- M1 is not delayed; pawaPay code lands in M3.

## D-012 Failure classes and slash timing (Nov 2, 2026)
Status: accepted (Kwame)
Context: An honest LP should not lose its bond to a provider outage, but an outage rule must not
let a dishonest LP keep a buyer's cedis. In the buy flow the cedis are with the LP off-chain, so
there is nothing to refund; the bond is the buyer's only remedy. An LP can also cause a run to
fail with `null` (delete or lock its secret), which looks exactly like an OutLayer outage.
Decision: every failure still ends in a slash if it never clears; failures are classed only to
set the time allowed (SPEC §4.5): `LP_FAULT` (key rejected/missing, key fingerprint mismatch) →
`T_retry` = 24 h; `OUTAGE` only (provider 5xx/timeouts/unclear answers/IN_RECONCILIATION,
OutLayer `null`, yield timeout) → `T_outage_max` = 72 h. Demo: 3 min and 6 min. Same windows for
sell-flow refunds, mirrored.
Why 72 h: covers outages of minutes to hours with a wide margin, a full-day provider or telco
incident, and one that starts on a Friday evening; the buyer is paid as soon as the provider
recovers, so 72 h is the worst case, not the normal wait. Shorter (48 h) risks slashing honest
LPs over a weekend incident; longer (7 days) leaves a buyer without cedis or USDC for a week and
keeps the LP's liquidity and bond locked. Why an LP gains nothing by faking an outage: it only
delays the same slash by 48 h. Residual risk: an outage longer than 72 h on an order the buyer
never paid; accepted and stated in the README. Values are deploy parameters copied into each
order (T20).
Alternatives: no slash during outages, refund the buyer instead (rejected: nothing to refund in
the buy flow, and it would excuse an LP that deletes its secret); one window for all failures
(rejected: harsher on honest LPs for no gain).

## D-013 near-contract-standards for the mock USDC token (Nov 3, 2026)
Status: accepted
Context: Escrow tests (unit now, sandbox in M1.8) need a NEP-141 token that behaves like USDC
(6 decimals, `ft_transfer_call` → `ft_on_transfer`, storage registration).
Decision: `contracts/mock-ft` uses `near-contract-standards` 5.30 (same repo and version as
near-sdk, https://github.com/near/near-sdk-rs), with an open `mint` for tests. TEST ONLY: never
deployed outside testnet/sandbox; the escrow's token is fixed at init (inv. 9), and on mainnet it
is native USDC.
Alternatives: hand-written NEP-141 (more code to get wrong); using real testnet USDC (not
mintable for tests).

## D-014 No owner role in the escrow (Nov 3, 2026)
Status: accepted (Kwame: "do what you have to do", reasoning below)
Context: SPEC v0.2 said an owner could set timing and cap parameters before orders exist. The
M1.1 scaffold stored an `owner` that did nothing. A reviewer asked us to pick one.
Decision: remove the owner. Every parameter is fixed at init, like the token and verifier.
Why (on its own merits, not because it was flagged): (1) the product claim is "nobody can change
the rules, not even us"; an owner with any power makes that claim conditional and is one more key
that can be stolen; (2) the only use was changing params before the first order, which is the same
as deploying a new instance with the right params, and a new instance gets a new `deploy_tag`
anyway; (3) a "before any order exists" guard needs extra state and tests, and gets subtle once
orders can be closed (zero open orders is not zero orders ever). Cost: a parameter mistake means
redeploying, which before launch is cheap.
Alternatives: owner can change params until the first order (rejected: weaker claim, more code,
no real benefit).

## D-015 Pinned Rust toolchain + reproducible contract builds (Nov 3, 2026)
Status: accepted
Context: near-sdk 5.30 needs Rust >= 1.95, and nothing pinned it. The wasm hashes recorded in M1.1
came from `cargo near build non-reproducible-wasm`, so nobody else could reproduce them.
Decision: `rust-toolchain.toml` pins Rust 1.97.0 (+ wasm32-unknown-unknown, wasm32-wasip2) at the
repo root. Both contracts carry `[package.metadata.near.reproducible_build]` with image
`sourcescan/cargo-near:0.22.0-rust-1.97.0` pinned by digest (from Docker Hub). Hashes that matter
(anything deployed or quoted to judges) come only from `cargo near build reproducible-wasm` on a
committed tree; local non-reproducible hashes are labelled as such.
Alternatives: pin only in the README (not enforced); skip reproducibility (judges cannot check that
deployed code matches the source).

## D-016 PR #7 review fix-ups: gas weighting, settings ceilings, 2+ failed attempts (Nov 3, 2026)
Status: accepted (Kwame: "go" on the fix-up)
Context: three read-only reviewers (security, docs-vs-code, contract code) checked the project.
Decisions, each for its own reason:
- Withdrawals: `ft_transfer` gets a 20 TGas minimum plus ALL unused gas (weight 1); `on_withdraw`
  gets a fixed 15 TGas (weight 0). With no owner and no setter, a fixed transfer budget that a
  token (e.g. native USDC) outgrows would block LP withdrawals forever; this way the LP can attach
  more gas. Measure real costs in the M1.8 sandbox.
- `update_quote` (and every future LP setting: pause/resume, secret change) requires 1 yoctoNEAR,
  so a leaked function-call key from the web app cannot reprice an LP's book (T34).
- Every callback checks `predecessor == current_account` in its body (as `new` does), because
  `#[private]` is invisible to unit tests.
- `min_failed_attempts` must be 2–10: with 1, "first and last failure a window apart" is never
  true, so a slash would be impossible. SPEC §7's 250 TGas cap on the OutLayer call is enforced.
- Ceilings on timing and fee (SPEC §7): settings are permanent, so a deployment with a 100-year
  outage window (no slash ever) must be impossible.
- `Lp.key_fp: Option<String>` exists now; its storage (68 bytes) is charged at registration, so the
  later fingerprint write does not drain the contract's own NEAR.
- Mock USDC: 1,000 mUSDC per mint. A per-mint cap alone makes the overflow-griefing attacks need
  ~3e29 mints, so no account deny-list is needed.
Alternatives: a setter for gas (rejected: re-introduces an owner); unbounded settings (rejected:
permanent mistakes).

## D-017 Design decisions before M1.4 (Nov 3, 2026)
Status: accepted (Kwame "agree" on items 1–6 of the three-reviewer review)
1. Pending payments are `PENDING` = `OUTAGE` class, never NOT_PAID: Paystack `pending`, `ongoing`,
   `processing`, `queued`, `send_otp`, `pay_offline`; pawaPay `ACCEPTED`, `ENQUEUED`, `PROCESSING`.
   They end at the same hard limit `T_outage_max`, so nobody can stall an order forever (T35, T36).
   Only failed / abandoned / reversed / not found are NOT_PAID.
2. One provider per LP: `"paystack"` (buy side) or `"pawapay"` (sell side). One verifier wasm
   dispatches on the provider; `key_fp` covers either key. The demo uses two LP accounts.
3. Checkout runs through the escrow (`request_checkout(order_id)`, contract-set fee), forced to
   GHS + `channels: ["mobile_money"]` (no cards, no chargebacks) + a synthetic email. The LP's
   secret is readable only via the escrow, so no other path works without loosening T19.
4. Rules the SPEC left open: `ghs_amount = ceil(usdc × rate / 1e6)` (round up, protects the LP),
   > 0 and ≤ u64; a buyer is new while they have fewer than `new_buyer_orders` RELEASED buy orders,
   cap per order; failed USDC transfers (release, slash, refund) go to a claimable balance anyone
   can retry; sell orders pre-pay storage with NEP-145 `storage_deposit`.
5. Only HTTP 401 is `LP_FAULT` (invalid key); 403 (WAF / outage) and unknown reasons are `OUTAGE`.
6. The `agents` track is dropped: a retry script is not an agent, and overselling it would hurt
   credibility. Focus on `realworld`.
Also: the books check (inv. 6, M1.7) is balance ≥ owed, not =, because anyone can send stray
mock USDC to the escrow. NEP-297 events and the provider limit come first in M1.3.

## D-018 Sell-order recipient commitment via pawaPay metadata (Q5) (Nov 3, 2026)
Status: accepted (Kwame, conditional on the sandbox proof, which passed)
Context: an unsalted or public-salt hash of a Ghana phone number (~3e9 candidates) is cracked in
seconds, so the salt must never be on-chain, but the TEE needs it to check the commitment.
Decision (SPEC §5): `recipient_commitment = sha256("openramp-sell-v1|" + salt + "|" + number + "|"
+ provider)`, salt 64 lowercase hex (fresh per order), number E.164 digits only, provider = pawaPay
code. The seller gives number + salt to the LP privately; the LP attaches the salt as the payout's
only metadata field (`openrampSalt`, `isPII: true`); the verifier reads it back from pawaPay with the
LP's key, requires COMPLETED, amount, the order's derived payoutId and exactly one valid metadata
field, and recomputes the hash from pawaPay's recipient phone + provider. The contract marks each
payoutId used. pawaPay and the LP see the salt; acceptable because both already know the number.
Evidence: sandbox payout 887bab2b-7ee9-4b75-8b4c-c3f70331daef, status reply
`"metadata": {"openrampSalt": "786047a8…67475"}`, identical to the salt sent (PROGRESS Nov 3).
Alternatives: salt as OutLayer input (rejected: on-chain, brute-forceable); per-order OutLayer
secret written by the LP (works, heavier; fallback if pawaPay metadata ever changes).

## D-019 PR process: reviewer subagents, then Claude merges (Nov 3, 2026)
Status: accepted (Kwame: "you can merge PRs going forward"; "after every PR you call subagents to
review")
Decision: for every PR, Claude runs `cargo test`, clippy and the build, opens the PR, runs reviewer
subagents (security + code/test), fixes their findings with tests, re-runs everything, waits for
GitHub checks, then merges. Kwame reviews afterwards. This replaces the AGENTS.md "ask before
merging unreviewed money logic" rule in practice; the AGENTS.md wording should be updated with
Kwame's approval of the diff.

## D-020 Buy-order storage, indexes and new-buyer history (PR #10 review) (Nov 4, 2026)
Status: accepted (reviewer findings, each fixed for its own reason)
- New-buyer status is counted per (lp, buyer), not globally: otherwise an attacker's own LP could
  release 3 penny orders to a fresh account and make it "experienced" for every honest LP,
  bypassing the new-buyer cap, exposure limit and release delay (T6, T29).
- Order storage charged at open includes what later steps write (checkout URL ≤ 256 chars, three
  timestamps, the first (lp, buyer) history entry). With no owner, nobody would top up the
  contract's NEAR, and running out would freeze every state change. Storage is kept for history,
  not refunded (THREAT T18 corrected).
- Per-account order indexes are `LookupMap<(account, position), id>` + a count, O(1) per order. A
  `Vec` rewritten on every open grew ~0.3–0.5 TGas per 1,000 past orders and would eventually make
  a popular or griefed LP impossible to order from.
- `ghs_amount_pesewas` is a U64 JSON string; buyers may pass `max_ghs_amount_pesewas` (slippage
  guard against a rate change just before open).
- No separate used-reference set: inv. 3 holds by construction (reference derived from the order
  id, verified only for its own order, terminal states).
Alternatives: global new-buyer counter weighted by volume (rejected: still gameable with one
colluding LP); refund storage on close (rejected: loses history and lowers the griefing cost).

## D-021 Verdict path schedules no transfers; no order can stay VERIFYING (PR #11 review) (Nov 4, 2026)
Status: accepted (both reviewers flagged the same High)
- PAID credits the buyer's claimable balance; `withdraw_claimable` (anyone, caller's gas) sends it.
  Reason: paying out from inside the verdict callback needed 35+ TGas of static gas and failed
  with the allowed callback budgets (measured: fails up to 52 TGas in the mock). A failed callback
  leaves the order in VERIFYING with no exit, freezing the LP's lock and bond. Now the PAID path
  fits in the 10 TGas minimum (tested).
- `VERIFY_TIMEOUT` = 15 min: after it, anyone may re-claim a VERIFYING order; the missing answer
  counts as an outage failure and a late callback is ignored (stale attempt).
- NOT_PAID for a check claimed before `pay_deadline` returns the order to AWAITING_PAYMENT with a
  claim window of at least `now + t_grace`: a slow answer must not expire an order paid on time
  (T38). A check claimed after the deadline can only find a late payment, so no extension.
Alternatives: raise the minimum callback gas to ~80 TGas (rejected: still fragile, and any other
panic would still freeze the order); always RELEASE_PENDING with `release_at = now` (works, but
needs a second call anyway, like the claimable balance, with one more state to reason about).
