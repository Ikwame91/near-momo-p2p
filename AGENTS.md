# AGENTS.md: read this fully before doing anything

## Mission
We are building **OpenRamp**: a trustless peer-to-peer on/off-ramp between mobile money and
stablecoins (USDC) on NEAR. Anyone can be a liquidity provider; USDC is released only on proof of
payment. First market for the demo: Ghana (MTN MoMo, Telecel Cash, AirtelTigo Money): Paystack test mode
for collections, pawaPay sandbox for payouts (DECISIONS D-011);
the payment provider is a plug-in so other countries/providers can be added. Built for the NEAR
Hackathon (build Nov 1–14, 2026; submit by Nov 14 23:59 UTC). Rules: https://nearhackathon.com/HACKATHON.md
Track: Real-World Money `realworld`. (`agents` dropped: no real agent feature, D-010, D-017.) Privacy
`privacy` only if we actually build something for it.
Owner: Kwame (all modules) for now.

## Architecture (do not change without a DECISIONS.md entry)
- `contracts/escrow`: NEAR smart contract (Rust, near-sdk). Holds USDC, manages orders, LP bonds,
  timeouts, bond slashing. Uses yield/resume to wait for the OutLayer verification result.
- `verifier/`: OutLayer program. Checks a mobile money payment via the payment provider API
  (Paystack test mode for collections, pawaPay sandbox for payouts; never live) inside the TEE. The verdict returns to the escrow contract through
  OutLayer's yield/resume callback; there is no separate signature to check unless OutLayer's docs
  show one (confirm in Test A). Provider is a plug-in behind an interface, so it can be swapped
  for a licensed partner.
- `settlement/`: NEAR Intents (1Click API) integration for cross-chain USDC in/out.
- `agent-api/` (DROPPED, DECISIONS D-010: needs real payouts): x402 endpoint so AI agents
  can pay a Ghanaian phone number in cedis. Needs the full sell flow plus Paystack payouts.
- `web/`: minimal frontend for buyer, seller and LP flows.
- `scripts/`: feasibility tests and dev scripts (`scripts/test-a`, `test-b`, `test-c`).

## Session protocol (EVERY session, no exceptions)
1. Read AGENTS.md, docs/SPEC.md, docs/PLAN.md, and the last 3 entries of docs/PROGRESS.md.
2. State in 3 lines: where we are, the single task you will do now, how you will verify it.
3. Do ONLY that task. If you discover other work, add it to PLAN.md; don't do it now.
4. Verify (tests/build/run) before claiming done. Paste the actual command output.
5. Update PLAN.md checkboxes and append a PROGRESS.md entry (done / failed / next).
6. Commit with a clear message and push. One logical change per commit.

## Engineering rules
- Small steps: change <~200 lines at a time, then test. Never rewrite a working module wholesale.
- Tests first for contract logic: write the failing test, then the code. Every invariant in SPEC.md
  has a test.
- Never mark a task done without running it. "Should work" is not done.
- Don't guess APIs. Read the docs or the SDK source, or write a tiny probe script. Cite the doc URL
  in a code comment for any external API call.
- Keep it boring: simple, explicit code over clever abstractions. No new dependency without a reason
  in DECISIONS.md.
- Errors are explicit: no silent catches; failing closed is the default for anything touching money.
- Config and secrets only via env vars. Never commit keys. `.env.example` lists every variable.
- Testnet and Paystack test mode by default. Mainnet only with an explicit human instruction and
  tiny amounts. Exception already approved: NEAR Intents has no testnet, so Test C and settlement
  tests use a dedicated mainnet dev account with ≤ $1 per transfer (see DECISIONS.md D-004).
- If stuck after 2 attempts on the same error: stop, write what you tried in PROGRESS.md, and ask.

## Money and security invariants (must always hold)
SPEC.md §8 is the full, canonical list; this is a summary.
- USDC is released only on a PAID verdict that arrives through the OutLayer callback for THIS
  order ID. Verdicts from any other caller or for any other order are rejected.
- "Could not verify" is NEVER treated as "not paid". After a retry window for real outages, a
  still-UNVERIFIABLE result slashes the LP's bond to the buyer. There is no dispute path and no
  admin or arbiter; adding one breaks the trustless claim.
- A payment reference can be used for exactly one order (replay protection).
- An LP cannot withdraw their bond or change their registered provider key while any order is open.
- If an LP revokes the key on Paystack's side, the check comes back UNVERIFIABLE and the bond is
  slashed under the rule above. The bond is the real defence against key revocation; never skip it.
- New-buyer amount cap and release delay are configurable settings (0 delay in the demo) to
  protect LPs from MoMo reversals; the README explains them.
- No phone numbers or API keys on-chain or in logs.

## Definition of done (per task)
Code + tests passing + PLAN.md ticked + PROGRESS.md entry + commit + push.

## Project history (required)
docs/PROJECT_HISTORY.md is the project's living record.
After completing any change that you commit:
- Add a dated Timeline entry: what changed, why, key files, commit hash.
- If you made a technical choice, add it to the Decisions log.
- If you added a shortcut or left something unfinished, add it to
  Known issues; remove items you fixed.
- Update the architecture section if the structure changed.
- Update "Last updated" with the date and latest commit hash.
Commit the history update together with the change itself.

## Git
- Push freely to this repo's working branches with clear commit messages.
- Ask before: merging to `main` from a feature branch with unreviewed money logic, or submitting.
  The repo is public (D-008): never commit secrets.
- No Claude co-author or session trailers in commit messages.

## Never
- Never delete or rewrite tests to make them pass.
- Never change the architecture, invariants or scope silently.
- Never touch mainnet, real money, or real user data without an explicit human go-ahead
  (except the approved Intents dev account above).
- Never post or submit anything (X post, hackathon form/API) without being asked.
