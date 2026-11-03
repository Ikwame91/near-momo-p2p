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
Alternatives: FeeRail; MoMo-for-agents via x402 (kept as stretch S1); AI Bursar.

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
Status: accepted (Kwame, option 3)
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
