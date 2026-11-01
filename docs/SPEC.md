# OpenRamp: Specification

Status: v0.2 APPROVED by Kwame (Nov 1, 2026). v0.2 adds the security fixes from the design audit
(DECISIONS D-009, THREAT_MODEL T23–T30).
Anything marked **[confirm in Test X]** is an assumption about an external API that a feasibility
test must prove before we build on it.

## 1. What it is
A trustless peer-to-peer on/off-ramp between mobile money and USDC on NEAR.
- **On-ramp (buy):** a buyer pays cedis by mobile money to a liquidity provider (LP) and receives
  USDC from escrow, on NEAR or any chain via NEAR Intents.
- **Off-ramp (sell):** a seller locks USDC in escrow, the LP pays them cedis by mobile money, and
  the USDC goes to the LP once the payout is proven.

Nobody has to trust the other side. The escrow contract only moves USDC on a verdict produced by
OutLayer code running in a TEE, which checks the payment directly with the payment provider using
the LP's API key (the key never leaves the TEE).

## 2. Actors
| Actor | Who | What they do |
|---|---|---|
| Buyer | Person or agent wanting USDC | Opens a buy order, pays mobile money, claims USDC |
| Seller | Person or agent wanting cedis | Locks USDC in a sell order, receives mobile money |
| LP | Liquidity provider (a merchant with a Paystack account) | Posts a bond and USDC liquidity, sets a rate, receives/sends mobile money |
| Verifier | OutLayer program in a TEE | Calls the provider API with the LP's secret key, returns PAID / NOT_PAID / UNVERIFIABLE |
| Escrow contract | NEAR contract `contracts/escrow` | Holds USDC and bonds, runs the order state machine |
| Settlement | NEAR Intents 1Click API | Moves released USDC to another chain if the user wants |

No admin or arbiter can move funds. The contract owner can only set timing and cap parameters
**before** orders exist that depend on them; see §7. The verifier program, the accepted token and the
OutLayer contract are fixed at deploy and can never be changed (invariant 9).

## 3. LP lifecycle
1. `register_lp(provider, secret_ref, rate_ghs_per_usdc, min, max, new_buyer_exposure_max)`:
   registers which OutLayer secret holds the LP's Paystack key and the LP's quote. The contract then
   runs the verifier once in `fingerprint` mode: inside the TEE it reads the key and returns
   `key_fp = sha256(key)`, which the contract stores. The LP takes no orders until `key_fp` is set.
   The LP stores the secret bound to the verifier wasm hash and readable only via the escrow
   (`--access allow-all --via <escrow>`, proven in Test A).
2. Deposit bond (USDC, `ft_transfer_call` with msg `bond`).
3. Deposit liquidity (USDC, msg `liquidity`) for buy orders.
4. Update rate/min/max at any time (applies to new orders only).
5. Withdraw liquidity that is not locked in an order: any time.
6. Withdraw bond or change `secret_ref` (which re-runs `fingerprint`): only when the LP has
   **zero open orders**.
7. `pause_lp()` / `resume_lp()`: a paused LP gets no new orders; open orders run to the end. This
   lets an LP reach zero open orders even if someone keeps opening orders (T28).

The LP *can* still change the key inside its OutLayer secret, or its access rule, at any time; the
contract cannot see that. It is caught instead: every verdict carries the key fingerprint, and a
mismatch, a refused secret or a failed run all count as UNVERIFIABLE, which ends in a slash (T24, T30).

Bond rule: the USDC value of an LP's open buy orders can never exceed their bond
(`open_buy_notional ≤ bond`), so a slash can always be paid in full.

## 4. Buy flow (cedis → USDC)
1. Buyer calls `open_buy_order(lp, usdc_amount)`. Contract checks the LP is not paused, LP limits,
   the bond rule, the per-account new-buyer cap and the LP's `new_buyer_exposure_max`, locks
   `usdc_amount` of LP liquidity, and computes `ghs_amount` from the LP rate. It issues `order_id`
   and the **payment reference** `opr-<deploy_tag>-<order_id>` (derived only by the contract, so it
   is bound to this order; `deploy_tag` = first 8 hex chars of
   `sha256(contract_id ‖ init_block_height)`, so references never repeat across deployments).
   State `AWAITING_PAYMENT`, `pay_deadline = now + T_pay`.
2. Buyer pays `ghs_amount` through a Paystack checkout with that reference. The checkout is created
   by the verifier in `checkout` mode, inside the TEE, with **the same LP key** that later verifies
   it (Paystack `POST /transaction/initialize`), so payment and check are always on one Paystack
   account (T24). **[confirm in M2]** whether this runs on-chain or via OutLayer's HTTPS API.
   The phone number is entered only on Paystack's page, never sent to the contract.
   Confirmed in Test A (M0.5): test mode accepts a custom reference for a mobile money charge.
3. Buyer (or anyone) calls `claim(order_id)` and attaches at least `outlayer_fee`. The **contract**
   sets the OutLayer deposit, gas and resource limits from its own config (extra is refunded), so a
   caller cannot make a verification fail by under-funding it (T25). Contract calls OutLayer
   `request_execution` with the fixed verifier source, input `{reference, expected_amount_pesewas,
   currency:"GHS", pay_deadline}` and the LP's `secret_ref`. State `VERIFYING`.
   Confirmed in Test A (M0.4–M0.5): call shape, deposit, secret access, callback delivery.
4. Verifier calls Paystack `GET /transaction/verify/:reference` with the LP key and returns
   `{verdict, reference, key_fp, reason}`. The contract accepts the verdict only if `reference` is
   this order's and `key_fp` equals the LP's registered one; otherwise it is UNVERIFIABLE. Verdicts:
   - `PAID` if status = success, amount ≥ expected, currency = GHS, reference matches, paid
     before `pay_deadline`.
   - `NOT_PAID` if Paystack answers clearly and any of the above fails (not found, pending,
     failed, short amount, late).
   - `UNVERIFIABLE` on auth error (key revoked/invalid), network/5xx error, timeout, or
     malformed response.
5. Callback (accepted only for this order's pending request):
   - `PAID` → record reference as used → release USDC to buyer (state `RELEASED`), or
     `RELEASE_PENDING` until `release_at` if a release delay applies.
   - `NOT_PAID` → back to `AWAITING_PAYMENT` if before `pay_deadline + T_grace`, else `EXPIRED`
     (LP liquidity unlocked).
   - `UNVERIFIABLE`, wrong reference or `key_fp`, OutLayer error (`null`), no value, or yield
     timeout → `UNVERIFIED`. All failures count: the contract cannot tell an outage from an LP
     that deleted or locked its secret, so it fails closed (T30). Anyone may `claim` again.
     Anyone may call `slash(order_id)` only when the order is still `UNVERIFIED` **and** it has
     at least `min_failed_attempts` failed results, the first and last at least `T_retry` apart
     (so a short outage cannot cause a slash). The buyer receives `usdc_amount` from the LP's
     bond, the locked liquidity is unlocked back to the LP. State `SLASHED`.
6. Optional: buyer withdraws released USDC to another chain via `settlement/` (1Click).

## 5. Sell flow (USDC → cedis), two-step
1. Seller sends USDC with `ft_transfer_call(msg: open_sell{lp, recipient_commitment})`, where
   `recipient_commitment = sha256(salt ‖ msisdn ‖ provider_code)` and `salt` is 32 random bytes the
   seller keeps (so the number cannot be brute-forced from the hash). Contract locks the USDC and
   issues `order_id`, reference `opr-<deploy_tag>-<order_id>`. State `AWAITING_PAYOUT`,
   `payout_deadline = now + T_pay`.
2. Seller gives the LP their mobile money number **off-chain** (through the web app to the LP's
   service), never on-chain. The salt reaches the verifier privately. **[confirm in M3, SPEC Q5]**
3. LP pays the seller with a Paystack transfer using that reference, then calls
   `claim_payout(order_id)` → OutLayer checks `GET /transfer/verify/:reference` **and** that the
   transfer's recipient matches `recipient_commitment` (T23).
   **[confirm in Test B]** test-mode mobile money transfers, transfer verify, recipient fields.
4. Outcomes:
   - Seller calls `confirm_received(order_id)` at any time → USDC to LP (`RELEASED`).
   - Verdict `PAID` (transfer success, amount, currency, reference, recipient matches the
     commitment, LP `key_fp` matches) → USDC to LP (`RELEASED`). Paying any other wallet, including
     the LP's own, is NOT_PAID.
   - `NOT_PAID` / `UNVERIFIABLE` → LP may retry until `payout_deadline + T_retry`; after that the
     seller can `refund(order_id)` → USDC back to seller (`REFUNDED`). The LP controls the key and
     the payout, so failure to prove the payout is the LP's risk.

## 6. Order states
```
BUY:  AWAITING_PAYMENT ─claim→ VERIFYING ─PAID→ RELEASED (or RELEASE_PENDING ─time→ RELEASED)
                 ↑                 │─NOT_PAID, before deadline+grace─┘(back)
                 │                 │─NOT_PAID, after → EXPIRED
                 │                 └─UNVERIFIABLE/err/timeout→ UNVERIFIED ─claim→ VERIFYING
                 │                                              └─T_retry passed, slash→ SLASHED
                 └─ no claim by deadline+grace, anyone calls expire → EXPIRED
SELL: AWAITING_PAYOUT ─confirm_received→ RELEASED
                      ─claim_payout→ VERIFYING ─PAID→ RELEASED
                                               └─other→ AWAITING_PAYOUT
                      ─deadline+T_retry passed, refund→ REFUNDED
```
Terminal states: `RELEASED`, `EXPIRED`, `SLASHED`, `REFUNDED`. Only one verification may be in
flight per order. A callback for a request that is not the order's current one is ignored.

## 7. Parameters (set at deploy; demo values in brackets)
| Name | Meaning | Prod idea | Demo |
|---|---|---|---|
| `T_pay` | time to pay / pay out | 30 min | 10 min |
| `T_grace` | extra time to claim after deadline | 15 min | 2 min |
| `T_retry` | how long UNVERIFIABLE may persist before slash/refund | 24 h | 3 min |
| `new_buyer_cap` | max USDC for an account's first N orders | 50 | 50 |
| `release_delay` | delay before release for new buyers (reversal protection) | 1 h | 0 |
| `min_failed_attempts` | failed verifications needed (spread over `T_retry`) before a slash | 3 | 2 |
| `outlayer_fee`, limits | deposit, gas and resource limits the contract uses for each verification | from `estimate_execution_cost` | same |
| `new_buyer_exposure_max` | per LP, set by the LP: total open USDC with new buyers | LP's choice | LP's choice |

Fixed at deploy, no setter exists (invariant 9): `token` (accepted USDC contract; native USDC on
mainnet, mock USDC on testnet), `verifier` (wasm URL + sha256 or OutLayer project + version),
`outlayer` contract id, `deploy_tag`.

## 8. Invariants (each one gets a test)
1. USDC is released only on a PAID verdict from the OutLayer callback for this order's current
   request (or on the seller's own `confirm_received` in a sell order).
2. UNVERIFIABLE is never treated as NOT_PAID. After `T_retry`, a buy order slashes the LP bond to
   the buyer; a sell order refunds the seller.
3. A payment reference settles at most one order.
4. An LP with open orders cannot withdraw their bond or change `secret_ref`.
5. `open_buy_notional ≤ bond` for every LP at all times.
6. Contract USDC balance ≥ sum of all bonds + liquidity + locked sell funds + pending releases.
7. No phone number or API key is stored on-chain or written to logs.
8. No account (including the owner) can move user funds outside the state machine.
9. The verifier source, accepted token and OutLayer contract are fixed at init and have no setter.
   `ft_on_transfer` accepts only the configured token. The deployed contract has no full-access key
   (or the README states plainly who can upgrade it).
10. A verdict counts only if it carries this order's reference and the LP's registered `key_fp`;
    anything else is UNVERIFIABLE.
11. The contract, not the caller, sets the OutLayer deposit, gas and limits. Every failure is
    UNVERIFIED, and a slash needs at least `min_failed_attempts` failures spanning `T_retry`.
12. A payment reference is unique across deployments (`opr-<deploy_tag>-<order_id>`).
13. A sell order is PAID only if the transfer went to the recipient the seller committed to.
14. A paused LP gets no new orders; an LP's open USDC with new buyers never exceeds its
    `new_buyer_exposure_max`.

## 9. Out of scope (hackathon)
- Real money on Paystack (live mode is banned for crypto businesses; we use test mode).
- Licensing (Ghana VASP Act 1154) and KYC; documented as the path to production.
- Refunds of mobile money paid after an order expired (handled off-chain by the LP).
- Price oracles; the LP sets its own rate.
- Partial fills, order books, LP reputation, multiple currencies besides GHS/USDC.
- Disputes, arbiters, admin overrides (by design).
- `agent-api/` x402 endpoint: stretch goal after the buy flow works end to end.

## 10. Open questions (resolve in M0)
- Q1 (ANSWERED in M0.4–M0.5, see PROGRESS.md): Exact OutLayer `request_execution` args, deposit, secret access control and callback format
  (Test A). Does the escrow contract or OutLayer own the yield? Mainnet OutLayer account id?
- Q2 (PARTLY ANSWERED, M0.6): mobile money charge with a custom reference works in test mode.
  Transfers to mobile money are BLOCKED on a Starter business account ("You cannot initiate third
  party payouts as a starter business"); they need a Registered Business. Decision pending.
- Q3: 1Click quote + withdraw of ~$1 USDC NEAR → Base, fees and time (Test C).
- Q4: Escrow on testnet (mock USDC) with Intents shown separately on mainnet, or escrow on mainnet
  with tiny amounts so release → Intents runs end to end? Decide after Test A and C (DECISIONS.md).
- Q5: How the sell-order `salt` (and the seller's number, if needed) reaches the verifier without
  going on-chain: encrypted input, or a seller-owned OutLayer secret? (M3)
- Q6: Can the verifier create the Paystack checkout (`checkout` mode) so the buyer gets the link
  without the key leaving the TEE: on-chain call or OutLayer HTTPS API? (M2)
- Q7: Can OutLayer compile the verifier from a GitHub commit, so anyone can check the wasm hash
  matches the source (reproducible build)? (M2)
