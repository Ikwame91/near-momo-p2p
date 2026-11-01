# OpenRamp: Threat Model

Status: v0.2 (Nov 1, 2026), adds T23–T30 from the design audit (DECISIONS D-009). Rule: no defence may rely on an admin or arbiter.
Each row maps to a test (filled in as tests are written).

| # | Attacker | Attack | Defence | Test |
|---|---|---|---|---|
| T1 | Buyer | Fake receipt / screenshot | The contract never trusts user evidence; only the verifier's Paystack lookup with the LP's key counts | — |
| T2 | Buyer | Reuse one real payment for two orders (replay) | Reference is `opr-<deploy_tag>-<order_id>`, issued by the contract (unique across deployments, T27); verifier checks the reference matches; contract records used references and rejects a second use | — |
| T3 | Buyer | Pay a smaller amount, or in another currency | Verifier checks amount ≥ expected and currency = GHS | paystack-verify `short_amount_is_not_paid`, `wrong_currency_is_not_paid` + live run (M0.5) |
| T4 | Buyer | Pay after the deadline, then claim | Verifier checks `paid_at ≤ pay_deadline`; late payments are refunded off-chain by the LP (out of scope) | paystack-verify `late_is_not_paid` + live run (M0.5) |
| T5 | Buyer | Claim during a Paystack outage hoping to slash the LP without paying | Outage gives UNVERIFIABLE, not PAID. Slash only after `T_retry` (24 h in prod) of continuous UNVERIFIABLE; anyone, including the LP, can re-claim during that window so a real outage clears. Each claim costs the OutLayer fee | — |
| T6 | Buyer | MoMo reversal after receiving USDC | New-buyer amount cap and `release_delay`; residual risk accepted by LPs and stated in README | — |
| T7 | Buyer | Open many orders without paying to lock LP liquidity (griefing) | Orders expire after `T_pay + T_grace`; new-buyer cap; optional small order fee (later) | — |
| T8 | LP | Receive cedis, then revoke/rotate the Paystack key so verification fails | Revocation → UNVERIFIABLE → after `T_retry` the bond pays the buyer in full (bond rule `open_buy_notional ≤ bond`) | live run with invalid key → UNVERIFIABLE `auth_error` (M0.5); slash test in M1.6 |
| T9 | LP | Withdraw bond or change `secret_ref` while orders are open | Contract blocks both while the LP has any open order. Changing the key *inside* the secret is T24 | — |
| T10 | LP | Change the rate after the buyer opened the order | Rate and `ghs_amount` are fixed in the order at open time | — |
| T11 | LP | Never pay out in a sell order but keep the seller's USDC | USDC only goes to the LP on PAID or seller confirmation; otherwise the seller refunds after the deadline | — |
| T12 | Seller | Receive cedis, then refuse to confirm | The LP proves the payout with `claim_payout`; the verdict releases USDC without the seller | — |
| T13 | Outsider | Call the callback directly with a fake PAID | Callback is `#[private]` (only the contract's own `.then` can deliver it) and must match the order's current request id. Confirmed in Test A (M0.4): direct call → "Method on_result is private" | test-a `on_result` (testnet); escrow test TBD |
| T14 | Outsider | Front-run: see a buyer's payment reference and claim it for their own order | Reference is tied to one order id; claiming another order with it fails the reference check | — |
| T15 | Anyone | Two claims in flight; callbacks arrive out of order | One verification in flight per order; stale callbacks ignored | — |
| T16 | OutLayer | Never responds / errors | Yield timeout or error → UNVERIFIED (never NOT_PAID, never release); retry allowed. Test A: failed run returns JSON `null`; OutLayer's timeout path returns no value (empty); oversized result → treat as no value | test-a `classify` unit tests |
| T17 | OutLayer/TEE | Compromised worker returns a false PAID | Trust assumption of the design (TEE attestation by OutLayer). Documented in README as a limitation | — |
| T18 | Anyone | Storage exhaustion by creating many orders | Caller pays storage deposit for each order; refunded when the order closes | — |
| T19 | Anyone | Leak phone numbers or keys | Phone numbers only on Paystack pages / LP service; keys only in OutLayer secrets, bound to the verifier wasm hash and readable only via the escrow (`--access allow-all --via <escrow>`); verifier prints only verdict/reference/reason; review prompt checks logs | M0.5: direct call to the LP's secret refused ("Access denied") |
| T21 | Anyone | Trigger verification with their own wasm that always prints PAID | The escrow fixes the verifier source (wasm URL + sha256, or OutLayer project + version) at deploy; callers can never choose it. Found in Test A: the probe's `request()` takes the URL from the caller, the escrow must not | — |
| T22 | OutLayer operator | The OutLayer operator account submits a false result (`resolve_execution` is operator-only) | Same trust assumption as T17 (TEE attestation); documented in README | — |
| T20 | Owner | Change parameters to trap open orders | Orders copy the parameters they were opened with; owner has no fund-moving methods | — |
| T23 | LP | Sell order: send the "payout" with the right reference and amount to the LP's own second wallet, get PAID, keep the seller's USDC | Seller commits `sha256(salt ‖ msisdn ‖ provider)` on-chain at open; verifier requires the transfer recipient to match; mismatch = NOT_PAID (SPEC §5, inv. 13) | — |
| T24 | LP | Buy order: take the buyer's money on Paystack account A but verify with a key for account B (or swap the key inside the secret), so the lookup says "not found" → NOT_PAID → order expires, LP keeps the cedis | `key_fp = sha256(key)` recorded by the TEE at registration; every verdict carries it, mismatch = UNVERIFIABLE → slash; the checkout is created inside the TEE with the same key (SPEC §3, §4, inv. 10) | — |
| T25 | Buyer | Make verification fail on purpose (too little deposit/gas, bad limits) so failures pile up and the LP gets slashed without the buyer paying | Contract sets deposit, gas and limits itself; slash needs `min_failed_attempts` failures spanning `T_retry` (inv. 11) | — |
| T26 | Owner | Swap the verifier for one that always says PAID, swap the token for a fake one, or redeploy the contract code | Verifier, token and OutLayer id fixed at init with no setter; `ft_on_transfer` checks the token; no full-access key on the contract, or the upgrade trust is stated in the README (inv. 9) | — |
| T27 | Buyer | Replay an old successful `opr-1` against a new deployment's order 1 (order ids restart), or across two instances sharing one Paystack account | `deploy_tag` from `sha256(contract_id ‖ init_block_height)` in every reference (inv. 12) | — |
| T28 | Anyone | Keep reopening orders against an LP so it never reaches zero open orders and can never withdraw its bond or rotate a leaked key | `pause_lp()` blocks new orders so the LP can drain to zero (inv. 14) | — |
| T29 | Buyer | Bypass the per-account new-buyer cap with many free NEAR accounts (reversal fraud) | Per-LP `new_buyer_exposure_max`, set by the LP (inv. 14) | — |
| T30 | LP | Make runs fail on purpose (delete the secret, change its access rule) so failures never count against it | The contract only sees `null` and cannot tell who caused a failure, so every failure counts as UNVERIFIED and ends in a slash after the retry window (fails closed) (SPEC §4.5, inv. 11) | — |
