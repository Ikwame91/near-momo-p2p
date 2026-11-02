# OpenRamp

A trustless peer-to-peer on/off-ramp between mobile money and USDC on NEAR. A NEAR escrow
contract releases USDC only when an OutLayer TEE has checked the mobile money payment directly
with the payment provider. First market: Ghana (MTN MoMo, Telecel Cash, AirtelTigo Money).

Built for the NEAR Hackathon (Nov 1–14, 2026). Work in progress: see `docs/SPEC.md` and `docs/PLAN.md`.

**Status:** testnet + Paystack test mode only. No real money.

**Buy flow (cedis → USDC) is real:** Paystack test-mode mobile money payments checked inside an
OutLayer TEE. **Sell flow (USDC → cedis): the payout check is simulated.** The contract logic is
built and tested, but a Paystack Starter business account cannot send transfers; real payouts need
a Paystack Registered Business or a licensed payout partner. See `docs/DECISIONS.md` D-010.
