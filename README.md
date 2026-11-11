# OpenRamp

A trustless peer-to-peer on/off-ramp between mobile money and USDC on NEAR. A NEAR escrow
contract releases USDC only when an OutLayer TEE has checked the mobile money payment directly
with the payment provider. First market: Ghana (MTN MoMo, Telecel Cash, AirtelTigo Money).

Built for the NEAR Hackathon (Nov 1–14, 2026). Work in progress: see `docs/SPEC.md` and `docs/PLAN.md`.

**Status:** testnet + Paystack test mode only. No real money.

**Build:** Rust is pinned by `rust-toolchain.toml` (1.97.0; near-sdk 5.30 needs >= 1.95), so
`rustup` installs the right version automatically. Contracts: `cd contracts && cargo test`;
reproducible wasm: `cd contracts/escrow && cargo near build reproducible-wasm` (needs Docker).

**Web page (buyer):** `cd web && npm install && npm run dev`, then open the printed address.
It talks to the testnet escrow in `web/.env.example` (override with a `web/.env.local`). Connect a
NEAR **testnet** wallet (Meteor or Intear) holding about 1 NEAR, open an order, get the Paystack
link, pay with test mobile money, then check the payment and withdraw. `?view=<account>` shows any
account's orders read-only. `npm test` runs the money-maths tests; `npm run build` writes the
static site to `web/dist`.

**Buy flow (cedis → USDC):** Paystack test-mode mobile money payments, checked inside an
OutLayer TEE. **Sell flow (USDC → cedis):** payouts run on the **pawaPay sandbox** (a Paystack
Starter account cannot send transfers), with a clearly labelled simulated check as fallback.
Real money needs licensed liquidity providers: payment providers require a registered business
and restrict crypto, and Ghana's VASP Act 2025 (Act 1154) requires a licence. In production,
OpenRamp is the trust layer between users and licensed LPs; users still never have to trust the
LP. See `docs/DECISIONS.md` D-010 to D-012.

**Buyer email (Paystack checkout):** Paystack requires a customer email to create a checkout
(`/transaction/initialize` rejects a request without one, and also rejects `.example` addresses).
The checkout request travels through a public NEAR transaction, so a real email there would be
personal data on-chain (SPEC §8 inv. 7). So:
- *Demo (built):* every checkout uses one synthetic address, `buyer@example.com` (a reserved
  domain with no inbox). Paystack's receipt email goes nowhere; the buyer's receipt is the
  on-chain order and its `order_released` event. The buyer types only their MoMo number, and
  only on Paystack's own checkout page, never in OpenRamp.
- *Real buyers (not built):* the buyer still never puts an email on-chain. The checkout uses a
  per-order alias derived from the payment reference (e.g. `opr-<tag>-<id>@receipts.<domain>`).
  A buyer who wants Paystack's receipt gives their real email once, off-chain, to the alias
  forwarder that the deployment runs. A licensed LP that already knows its customers (KYC)
  can map the alias to the customer in its own records instead. Either way the contract and
  the verifier only ever see the alias.
