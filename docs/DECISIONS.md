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
