# OpenRamp

A trustless peer-to-peer on/off-ramp between mobile money and USDC on NEAR. A NEAR escrow
contract releases USDC only when an OutLayer TEE has checked the mobile money payment directly
with the payment provider. First market: Ghana (MTN MoMo, Telecel Cash, AirtelTigo Money).

Built for the NEAR Hackathon (Nov 1–14, 2026). Work in progress: see `docs/SPEC.md` and `docs/PLAN.md`.

**Status:** testnet + Paystack test mode only. No real money.

**Buy flow (cedis → USDC):** Paystack test-mode mobile money payments, checked inside an
OutLayer TEE. **Sell flow (USDC → cedis):** payouts run on the **pawaPay sandbox** (a Paystack
Starter account cannot send transfers), with a clearly labelled simulated check as fallback.
Real money needs licensed liquidity providers: payment providers require a registered business
and restrict crypto, and Ghana's VASP Act 2025 (Act 1154) requires a licence. In production,
OpenRamp is the trust layer between users and licensed LPs; users still never have to trust the
LP. See `docs/DECISIONS.md` D-010 to D-012.
