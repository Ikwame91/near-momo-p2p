# OpenRamp: Specification

Status: v0.4 APPROVED by Kwame (Nov 3, 2026). v0.2 added the design-audit fixes (D-009, T23–T30);
v0.3 failure classes (D-012) and pawaPay sandbox payouts (D-011); v0.4 no owner (D-014), review
fix-ups (D-016), pending = outage, one provider per LP, checkout through the escrow, sell-order
salt via pawaPay metadata (D-017, D-018).
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
| LP | Liquidity provider with ONE provider account: Paystack (buy-side LP, receives cedis) or pawaPay (sell-side LP, pays cedis) (D-017) | Posts a bond and USDC liquidity, sets a rate, receives or sends mobile money |
| Verifier | OutLayer program in a TEE | Calls the provider API with the LP's secret key, returns PAID / NOT_PAID / UNVERIFIABLE |
| Escrow contract | NEAR contract `contracts/escrow` | Holds USDC and bonds, runs the order state machine |
| Settlement | NEAR Intents 1Click API | Moves released USDC to another chain if the user wants |

No admin, arbiter or owner exists (D-014). Every setting (token, OutLayer contract, verifier,
timing, caps and OutLayer call limits) is fixed when the contract is initialised and can never be
changed by anyone, including the deployer (invariant 9). Different settings mean deploying a new
instance, which gets its own `deploy_tag`. Only the contract's own account can initialise it,
in the same transaction as the deploy (invariant 9, T32).

## 3. LP lifecycle
1. `register_lp(provider, secret_ref, quote{rate_pesewas_per_usdc, min_usdc, max_usdc,
   new_buyer_exposure_max})`: `provider` is exactly `"paystack"` (buy-side LP) or `"pawapay"`
   (sell-side LP); buy orders need a paystack LP, sell orders a pawapay LP (D-017). Registers which
   OutLayer secret holds the LP's provider key (Paystack secret key or pawaPay API token) and the
   LP's quote. The secret must be the LP's own (`secret_ref.account_id` = the LP; T33). One
   verifier wasm serves both providers and dispatches on `provider`. The contract then
   runs the verifier once in `fingerprint` mode: inside the TEE it reads the key and returns
   `key_fp = sha256(key)` (for either provider's key), which the contract stores. The LP takes no
   orders until `key_fp` is set. Every change to the LP's settings requires 1 yoctoNEAR (T34).
   The LP stores the secret bound to the verifier wasm hash and readable only via the escrow
   (`--access allow-all --via <escrow>`, proven in Test A).
2. Deposit bond (USDC, `ft_transfer_call` with msg `bond`).
3. Deposit liquidity (USDC, msg `liquidity`) for buy orders.
4. Update rate/min/max at any time (applies to new orders only).
5. Withdraw liquidity that is not locked in an order: any time.
6. Withdraw bond or change `secret_ref` (`set_secret_ref`, 1 yocto; clears `key_fp` so the new key
   must be fingerprinted before new orders): only when the LP has **zero open orders**.
   Registration pays storage for the longest profile name (64 chars) and the `key_fp`, so these
   changes never grow unpaid storage.
   LP actions emit NEP-297 events (`standard: "openramp"`, version `1.0.0`, `data` is an object,
   pots are `"bond"` / `"liquidity"`): `lp_registered`, `deposit`, `withdraw_requested`,
   `withdraw_recredited`, `quote_updated`, `secret_ref_changed`, `lp_paused`, `lp_resumed`. Events
   never contain phone numbers, keys or salts. Profile names (A-Z a-z 0-9 - _ only) and `key_fp`
   are public anyway (call arguments and `get_lp`); events leave the profile out only to stay
   minimal. The protection of the LP's key is the secret's access rule (`--via <escrow>`, bound to
   the verifier hash), never the secrecy of its name (T19).
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
   `usdc_amount` of LP liquidity, and computes `ghs_amount_pesewas = ceil(usdc_amount ×
   rate_pesewas_per_usdc / 1_000_000)` with checked maths (rounded **up**, protecting the LP); it
   must be > 0 and fit in u64. A buyer is **new with an LP** while they have fewer than
   `new_buyer_orders` RELEASED buy orders **with that LP** (per (lp, buyer), D-020: a colluding LP
   cannot "graduate" a buyer for honest LPs); for a new buyer each order is capped at
   `new_buyer_cap` and counts towards that LP's `new_buyer_exposure_max` (D-017). The buyer may pass
   `max_ghs_amount_pesewas` to refuse a rate raised just before the order opens. Storage charged at
   open covers what the order writes now **and** what later steps will write (checkout URL ≤ 256
   chars, verification timestamps, the first (lp, buyer) history entry; D-020); it is kept for the
   order's history, not refunded. Per-account order indexes cost the same for every order. It
   issues `order_id`
   and the **payment reference** `opr-<deploy_tag>-<order_id>` (derived only by the contract, so it
   is bound to this order; `deploy_tag` = first 8 hex chars of
   `sha256(contract_id ":" init_block_height)` with the height in decimal, e.g.
   `sha256("escrow.testnet:100")`, so references never repeat across deployments).
   State `AWAITING_PAYMENT`, `pay_deadline = now + T_pay`. The buyer attaches NEAR for the order's
   storage (excess refunded); an LP cannot buy from itself. Anyone may call `expire_order` once
   `now > pay_deadline + T_grace` while the order still awaits payment. Views: `get_order`,
   `list_orders_by_lp` / `list_orders_by_buyer` (pages of ≤ 50), `get_buyer_released`. Timestamps
   and amounts (incl. `ghs_amount_pesewas`) are JSON strings (`U64` / `U128`), because ns
   timestamps exceed JavaScript's 2^53. `count_orders_by_lp` / `count_orders_by_buyer` let the app
   page from the newest end.
   Events: `order_opened`, `order_expired`.
2. Buyer calls `request_checkout(order_id)` (fee set by the contract, as in step 3). The escrow
   runs the verifier in `checkout` mode inside the TEE with **the same LP key** that later verifies
   it (Paystack `POST /transaction/initialize` with the order's reference and amount,
   `currency: "GHS"`, `channels: ["mobile_money"]` so no card payments or chargebacks, and a fixed
   synthetic email, no PII); the callback stores the returned `authorization_url` on the order.
   It must go through the escrow because the LP's secret is readable only via the escrow (D-017,
   answers Q6). Buyer pays `ghs_amount` on that Paystack page.
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
   - `NOT_PAID` if Paystack answers clearly and any of the above fails: not found, `failed`,
     `abandoned`, `reversed`, short amount, wrong currency, late.
   - `PENDING` (verifier reason `pending`) if the charge is still in flight (`pending`, `ongoing`,
     `processing`, `queued`, `send_otp`, `pay_offline`, …). Treated as `OUTAGE`, never as NOT_PAID,
     because the buyer may have approved on the phone and the money can still arrive (T35, D-017).
   - `UNVERIFIABLE` on auth error (401: key revoked/invalid), network/5xx error, timeout, 403 (WAF /
     outage, not LP fault), or malformed response.
5. Callback `on_verdict(order_id, attempt, result)` (`#[private]` + caller check), accepted only
   while VERIFYING and only for the order's current `attempt` (a late callback for an older claim
   is ignored, T15). `claim(order_id)` is open to anyone while AWAITING_PAYMENT up to and including
   `pay_deadline + t_grace`, or from UNVERIFIED at any time; each claim increments `attempt`.
   `result` is the verifier output `{verdict, reference, key_fp, reason}` or `RunFailed` (OutLayer
   `null` / no value / timeout). Implemented in M1.5 with this typed input; M2.2 parses OutLayer's
   response into it:
   - `PAID` → record reference as used → release USDC to buyer (state `RELEASED`), or
     `RELEASE_PENDING` until `release_at` if a release delay applies; after that anyone may call
     `finalize_release(order_id)`. If the USDC transfer to the recipient fails (e.g. not registered
     on the token), the amount is credited to a **claimable balance** for that account, which
     anyone may retry with `withdraw_claimable(account)`; the books stay exact (inv. 6, D-017). The
     same applies to slash payouts and seller refunds.
   - `NOT_PAID` → back to `AWAITING_PAYMENT` if before `pay_deadline + T_grace`, else `EXPIRED`
     (LP liquidity unlocked).
   - `UNVERIFIABLE`, wrong reference or `key_fp`, OutLayer error (`null`), no value, or yield
     timeout → `UNVERIFIED`. **Every failure ends in a slash if it never clears** (the buyer's
     cedis are with the LP, off-chain; the bond is the only way to repay her, T30). Failures are
     sorted into two classes (D-012), which only change *how long* the LP gets:

     | Class | Results | Slash window |
     |---|---|---|
     | `LP_FAULT` | verifier `auth_error` (HTTP 401 only) / `no_key` (key revoked or missing), `key_fp` mismatch | `T_retry` |
     | `OUTAGE` | verifier `pending`, `server_error`, `network_error`, 403, `unclear_4xx`, `malformed`, `unexpected_status`, any unknown reason; provider IN_RECONCILIATION or pending; OutLayer `null`, no value, yield timeout | `T_outage_max` |

     Pending payments end at the same hard limit `T_outage_max`, so neither side can stall an
     order forever (D-017).

     If any failure in the current unverified streak is `LP_FAULT`, the window is `T_retry`;
     otherwise `T_outage_max`. A buyer cannot produce an `LP_FAULT` result. An LP can make a run
     fail with `null` (by deleting or locking its secret), but that only buys time up to
     `T_outage_max`. Anyone (including the LP) may `claim` again at any time; a PAID result
     releases and a NOT_PAID result after the deadline expires the order, ending the streak.
     Anyone may call `slash(order_id)` only when the order is still `UNVERIFIED` **and** it has
     at least `min_failed_attempts` failed results, the first and last at least the window apart.
     The buyer receives `usdc_amount` from the LP's bond, the locked liquidity is unlocked back to
     the LP. State `SLASHED`.
6. Optional: buyer withdraws released USDC to another chain via `settlement/` (1Click).

## 5. Sell flow (USDC → cedis), two-step
**Demo note (D-010, D-011):** the contract logic below is built and tested in full. Payouts run
on the **pawaPay sandbox** (Paystack Starter accounts cannot send transfers); the simulated
provider stays as a fallback in a separate, clearly labelled verifier build. pawaPay specifics:
- `payoutId` must be a UUID, so the contract derives one per order:
  `payout_id = uuid_from(sha256("payout" ‖ deploy_tag ‖ order_id))`. pawaPay ignores a reused id
  (`DUPLICATE_IGNORED`), which also blocks replays.
- Phone numbers are committed and compared in one canonical form: `233` + 9 digits, no `+`,
  no leading 0 (e.g. `233593456789`).
- Amounts are strings; pawaPay may echo a different format than sent (Test B2: sent `"5"`,
  status returned `"5.00"`), so the verifier converts both to pesewas and compares numbers.
- Status mapping (`GET /v2/payouts/{payoutId}`, https://docs.pawapay.io/v2/api-reference/payouts/check-payout-status):
  `COMPLETED` + amount, currency, payoutId, recipient commitment (below) and `key_fp` match → PAID;
  `FAILED`, `NOT_FOUND`, or any mismatch → NOT_PAID; `ACCEPTED` / `ENQUEUED` / `PROCESSING` →
  `PENDING` = `OUTAGE` class, never NOT_PAID (otherwise the seller could refund while the payout is
  still in flight and keep both, T36, D-017); `IN_RECONCILIATION`, 5xx → `OUTAGE`; 401 → `LP_FAULT`.

**Recipient commitment (Q5, D-018).** The seller's number and salt never go on-chain:
1. Hash input, unambiguous and versioned:
   `recipient_commitment = sha256("openramp-sell-v1|" + salt + "|" + number + "|" + provider)` where
   `salt` = 64 lowercase hex chars (32 random bytes), `number` = E.164 digits only, no `+`
   (e.g. `233593456789`), `provider` = pawaPay provider code (`MTN_MOMO_GHA`, `VODAFONE_GHA`,
   `AIRTELTIGO_GHA`). The web app generates a **fresh salt for every order**.
2. The seller gives the LP service the number and salt privately (web app → LP service). The LP
   pays with pawaPay and attaches the salt as the payout's **only** metadata field,
   `[{"openrampSalt": <salt>, "isPII": true}]`.
3. The verifier reads `GET /v2/payouts/{payoutId}` with the LP's key and checks: `COMPLETED`,
   amount and currency, that `payoutId` is the one derived from this order, **exactly one**
   metadata field named `openrampSalt` holding 64 lowercase hex (anything else → NOT_PAID), and
   `sha256("openramp-sell-v1|" + salt + "|" + recipient.phoneNumber + "|" + recipient.provider)`
   = the on-chain commitment. Proven in the sandbox (PROGRESS Nov 3): the status reply returns the
   metadata in clear as an object, `"metadata": {"openrampSalt": "<salt>"}`.
4. The contract marks each `payoutId` as used (one payout settles one order, like inv. 3), and
   `key_fp` covers the pawaPay token (§3.1).
5. pawaPay and the LP can see the salt. Accepted: both already know the seller's number, which is
   all the salt protects.
1. Seller sends USDC with `ft_transfer_call(msg: {"open_sell": {lp, recipient_commitment}})`
   (JSON msg), with the commitment defined above. Contract locks the USDC and
   issues `order_id`, reference `opr-<deploy_tag>-<order_id>`. State `AWAITING_PAYOUT`,
   `payout_deadline = now + T_pay`.
2. Seller gives the LP service their number and salt **off-chain** (web app → LP service), never
   on-chain.
3. LP pays the seller with a pawaPay payout using the order's derived `payoutId` and the salt
   metadata, then calls `claim_payout(order_id)` → the verifier checks the payout as above (T23).
   Confirmed in Test B2 (M0.6b): sandbox payouts, status, recipient phone + provider.
4. Outcomes:
   - Seller calls `confirm_received(order_id)` at any time → USDC to LP (`RELEASED`).
   - Verdict `PAID` (as defined above) → USDC to LP (`RELEASED`). Paying any other wallet,
     including the LP's own, is NOT_PAID.
   - `NOT_PAID` / `UNVERIFIABLE` / `PENDING` → LP may retry until `payout_deadline + T_retry`, or
     `payout_deadline + T_outage_max` **instead** if every failure was `OUTAGE` (so an honest LP who
     paid is not refunded against during a provider outage); after that the seller can
     `refund(order_id)` → USDC back to seller (`REFUNDED`). The LP controls the key and the payout,
     so failure to prove the payout is the LP's risk.
   - Sell orders arrive through `ft_transfer_call`, which carries no NEAR, so the seller pre-pays
     order storage with NEP-145 `storage_deposit` on the escrow (D-017).

## 6. Order states
```
BUY:  AWAITING_PAYMENT ─claim→ VERIFYING ─PAID→ RELEASED (or RELEASE_PENDING ─time→ RELEASED)
                 ↑                 │─NOT_PAID, before deadline+grace─┘(back)
                 │                 │─NOT_PAID, after → EXPIRED
                 │                 └─UNVERIFIABLE/err/timeout→ UNVERIFIED ─claim→ VERIFYING
                 │                                              └─window (§4.5) passed, slash→ SLASHED
                 └─ no claim by deadline+grace, anyone calls expire → EXPIRED
SELL: AWAITING_PAYOUT ─confirm_received→ RELEASED
                      ─claim_payout→ VERIFYING ─PAID→ RELEASED
                                               └─other→ AWAITING_PAYOUT
                      ─deadline+window (§4.5) passed, refund→ REFUNDED
```
Terminal states: `RELEASED`, `EXPIRED`, `SLASHED`, `REFUNDED`. Only one verification may be in
flight per order. A callback for a request that is not the order's current one is ignored.

## 7. Parameters (set at deploy; demo values in brackets)
| Name | Meaning | Prod idea | Demo |
|---|---|---|---|
| `T_pay` | time to pay / pay out | 30 min | 10 min |
| `T_grace` | extra time to claim after deadline | 15 min | 2 min |
| `T_retry` | slash/refund window when a failure is the LP's fault (`LP_FAULT`) | 24 h | 3 min |
| `T_outage_max` | slash/refund window when every failure is an outage (`OUTAGE`); hard limit | 72 h | 6 min |
| `new_buyer_cap` | max USDC per order for a new buyer (§4.1) | 50 | 50 |
| `new_buyer_orders` | RELEASED buy orders after which a buyer is no longer new (1–100) | 3 | 3 |
| `release_delay` | delay before release for new buyers (reversal protection) | 1 h | 0 |
| `min_failed_attempts` | failed verifications needed (spread over the window) before a slash; 2–10 (1 could never span a window) | 3 | 2 |
| `outlayer_call` | per verification: `fee` (yoctoNEAR deposit), `gas_tgas` (100–250), `callback_gas_tgas` (≥ 10; gas + callback ≤ 280), `max_instructions` (≤ 500e9), `max_memory_mb` (≤ 4096), `max_execution_seconds` (≤ 180) | fee from `estimate_execution_cost` | fee 0.01 NEAR, 200 + 30 TGas, 1e9 instr, 128 MB, 20 s |
| `new_buyer_exposure_max` | per LP, set by the LP: total open USDC with new buyers | LP's choice | LP's choice |

Everything above and below is fixed at deploy; no setter exists (invariant 9, D-014): `token`
(accepted USDC contract; native USDC on mainnet, mock USDC on testnet), `verifier` (wasm URL +
sha256 or OutLayer project + version), `outlayer` contract id, `deploy_tag`, and all parameters
in the table. `new` rejects: token = outlayer, either = the contract itself, a non-https verifier
URL, a hash that is not 64 lowercase hex chars, zero `t_pay` / `t_retry` / fee / `new_buyer_cap`,
`t_outage_max < t_retry`, `min_failed_attempts` outside 2–10, OutLayer limits outside the ranges
above, and values over the ceilings (D-016): `t_pay` and `t_grace` ≤ 24 h, `t_retry` and
`t_outage_max` ≤ 7 days, `release_delay` ≤ 24 h, OutLayer fee ≤ 1 NEAR. Because settings are
permanent, the ceilings stop a deployment whose slash can never happen in practice.

## 8. Invariants (each one gets a test)
1. USDC is released only on a PAID verdict from the OutLayer callback for this order's current
   request (or on the seller's own `confirm_received` in a sell order).
2. UNVERIFIABLE and PENDING are never treated as NOT_PAID. After the slash/refund window (§4.5:
   `T_retry` or `T_outage_max`), a buy order slashes the LP bond to the buyer; a sell order refunds
   the seller.
3. A payment reference settles at most one order. Holds by construction: the reference is derived
   from the order id, each verification checks only its own order's reference, and a settled order
   is terminal, so no separate used-reference set is stored (D-020).
4. An LP with open orders cannot withdraw their bond or change `secret_ref`.
5. `open_buy_notional ≤ bond` for every LP at all times.
6. Contract USDC balance ≥ sum of all bonds + liquidity + locked funds + pending releases +
   claimable balances. "≥", not "=", because anyone can send stray tokens to the escrow.
7. No phone number or API key is stored on-chain or written to logs.
8. No account (including the deployer) can move user funds outside the state machine.
9. The verifier source, accepted token, OutLayer contract and all parameters are fixed at init and
   have no setter; there is no owner role. Only the contract's own account can call `new`.
   `ft_on_transfer` accepts only the configured token. The deployed contract has no full-access key
   (or the README states plainly who can upgrade it).
10. A verdict counts only if it carries this order's reference and the LP's registered `key_fp`;
    anything else is UNVERIFIABLE.
11. The contract, not the caller, sets the OutLayer deposit, gas and limits. Every failure is
    UNVERIFIED and ends in a slash if it never clears. A slash needs at least
    `min_failed_attempts` failures spanning `T_retry` (any `LP_FAULT` failure) or `T_outage_max`
    (only `OUTAGE` failures). No failure class ever releases USDC or skips the slash.
12. A payment reference is unique across deployments (`opr-<deploy_tag>-<order_id>`).
13. A sell order is PAID only if the payout went to the recipient the seller committed to, and each
    pawaPay `payoutId` settles at most one order.
14. A paused LP gets no new orders; no new-buyer order is accepted that would take the LP's open
    USDC with new buyers above its `new_buyer_exposure_max` (the LP may lower the limit below the
    current exposure; that only blocks further new-buyer orders).

## 9. Out of scope (hackathon)
- Real money on Paystack (live mode is banned for crypto businesses; we use test mode).
- Licensing (Ghana VASP Act 1154) and KYC; documented as the path to production.
- Refunds of mobile money paid after an order expired (handled off-chain by the LP).
- Price oracles; the LP sets its own rate.
- Partial fills, order books, LP reputation, multiple currencies besides GHS/USDC.
- Disputes, arbiters, admin overrides (by design).
- `agent-api/` x402 endpoint: DROPPED (D-010), needs real payouts.
- Real mobile money payouts: payouts run on the pawaPay sandbox in the demo (D-011), with the
  simulated provider as fallback. Real money on any provider needs a licensed LP: Paystack and
  pawaPay both require a legal business and ban or restrict crypto/unlicensed money transfer, and
  Ghana's VASP Act 2025 (Act 1154) requires a licence for crypto services to the public. In
  production OpenRamp is the trust layer between users and licensed LPs (D-011).

## 10. Open questions (resolve in M0)
- Q1 (ANSWERED in M0.4–M0.5, see PROGRESS.md): Exact OutLayer `request_execution` args, deposit, secret access control and callback format
  (Test A). Does the escrow contract or OutLayer own the yield? Mainnet OutLayer account id?
- Q2 (ANSWERED, M0.6, M0.6b, D-010, D-011): mobile money charge with a custom reference works in
  Paystack test mode; Paystack transfers are blocked on a Starter account, so payouts run on the
  pawaPay sandbox (Test B2 passed).
- Q3: 1Click quote + withdraw of ~$1 USDC NEAR → Base, fees and time (Test C).
- Q4: Escrow on testnet (mock USDC) with Intents shown separately on mainnet, or escrow on mainnet
  with tiny amounts so release → Intents runs end to end? Decide after Test A and C (DECISIONS.md).
- Q5 (ANSWERED, D-018): the salt travels inside the pawaPay payout as its only metadata field and
  the verifier reads it from pawaPay (§5). Proven in the sandbox on Nov 3.
- Q6 (ANSWERED, D-017): checkout runs through the escrow (`request_checkout`), because the LP's
  secret is readable only via the escrow.
- Q7: Can OutLayer compile the verifier from a GitHub commit, so anyone can check the wasm hash
  matches the source (reproducible build)? (M2)
