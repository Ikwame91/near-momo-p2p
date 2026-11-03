# Progress log

Newest entry at the top. Template:
```
## YYYY-MM-DD HH:MM UTC — <session / task>
Done:
Failed / blocked:
Next:
Gotchas:
```

## 2026-11-03 — Session 3: M1.2 DONE (LP registration, deposits, withdrawals)
Branch: `escrow/lp-bonds-and-deposits` (from main).
Done: `contracts/escrow/src/lp.rs` + `lp_tests.rs`:
- `register_lp(provider, secret_ref, quote)`: caller pays the storage of its record (refund of the
  excess), rejects double registration, bad quotes (rate 0, min 0, min > max), bad names, and a
  `secret_ref` that is not the LP's own (new T33, found in self-review).
- `update_quote` (new orders only). `get_lp` view.
- `ft_on_transfer(sender, amount, msg)`: only the configured token (inv. 9, T26); msg "bond" or
  "liquidity"; amount > 0; sender must be a registered LP. Every rejection panics so NEP-141
  refunds the full amount.
- `withdraw_liquidity` / `withdraw_bond`: exactly 1 yoctoNEAR, only available balance, bond only
  with zero open orders (inv. 4; exercised with real orders in M1.3). Debit first, `ft_transfer`,
  then `on_withdraw` (#[private]) re-credits if the transfer failed.
```
cargo test (contracts/): escrow 51 passed (28 config + 23 LP); mock-ft 3 passed; 0 failed, 0 warnings
cargo near build non-reproducible-wasm (escrow): OK (local only, not reproducible; D-015)
```
Failed / blocked: none. First run had 4 must-use warnings (detached refund promise, test helpers);
fixed with `.detach()` (near-sdk 5.30 `Promise::detach`).
Next: M1.3 bond/secret lock while orders open, bond rule, `pause_lp`.

## 2026-11-03 — Session 3: M1.1 fix-up after review of PR #5
Done (each change has its reason in the code comment or DECISIONS):
- No owner role (D-014): removed `owner`; all params fixed at init; SPEC §2, §7, inv. 8–9, T20, T26.
- `new` callable only by the contract account (T32, found in this review): explicit `require!`
  in the body, because `#[private]` is enforced only in the wasm entry point and a unit test
  calling `new` directly did NOT panic (the test caught that). PLAN M2.7: deploy + init in one tx.
- `Params.outlayer_call` (inv. 11): fee, gas 100–250 TGas, callback ≥ 10 TGas, total ≤ 280,
  instructions ≤ 500e9, memory ≤ 4096 MB, seconds ≤ 180 (OutLayer docs maximums).
- Init rejects token = outlayer, token/outlayer = the contract itself.
- deploy_tag formula `sha256(contract_id ":" height)` now identical in code, SPEC §4.1, T27, with a
  test against the literal formula.
- Tests added: zero t_pay / t_retry / fee, low/high gas, instruction/memory/second bounds and
  maximums, empty Project id/version, valid Project, UPPERCASE 64-char hash, non-hex 64-char hash.
- mock-ft: minting to a NEW account requires the storage deposit (stops draining its NEAR);
  `ft_resolve_transfer` emits FtBurn when a refund is burned. 2 new tests.
- `rust-toolchain.toml` (1.97.0), reproducible-build metadata in both Cargo.toml files (D-015),
  README build note. Old local hashes above labelled "not reproducible".
```
cargo test (contracts/): escrow 28 passed; mock-ft 3 passed; 0 failed, 0 warnings
```
Failed / blocked: first run had 1 failure (`init_by_another_account_is_rejected`: `#[private]` not
visible to unit tests); fixed by the explicit check above.
Reproducible build attempt (commit c3e90b5, `cargo near build reproducible-wasm`, dockerd started
in the container): git clean check + image digest OK, then `docker image pull
sourcescan/cargo-near:0.22.0-rust-1.97.0@sha256:b193c864…` → Docker Hub `429 Too Many Requests`
(anonymous pull rate limit). Tried twice; stopped per the 2-attempt rule. Not a config problem.
Next: retry the reproducible build in a later session (before M2.7 deploy, PLAN M2.7); if Docker
Hub keeps rate-limiting, Kwame adds Docker Hub credentials as a secret. Then Test B2 or M1.2.

## 2026-11-03 — Session 3: M1.1 DONE (escrow scaffold + mock USDC)
Branch: `escrow/scaffold-and-mock-usdc` (from main after PR #4).
Done: `contracts/` cargo workspace (shared release profile).
- `contracts/escrow`: `new(owner, token, outlayer, verifier, params)` stores the deploy-time config
  with no setters (inv. 9); `deploy_tag` = first 8 hex of sha256(contract_id ":" block_height)
  (inv. 12, T27); validates verifier (https URL + 64-hex sha256, or project+version) and params
  (t_outage_max >= t_retry, min_failed_attempts >= 1, outlayer_fee > 0); `get_config` view.
- `contracts/mock-ft`: TEST ONLY NEP-141 mUSDC, 6 decimals, open `mint` (D-013).
```
cargo test (contracts/):
  escrow: 6 passed (init_stores_fixed_config, deploy_tag_is_8_hex_and_differs_per_deployment,
          rejects_bad_verifier_hash, rejects_http_verifier_url,
          rejects_outage_window_shorter_than_retry, rejects_zero_failed_attempts)
  mock-ft: 1 passed (mint_and_transfer)
cargo near build non-reproducible-wasm (LOCAL ONLY, not reproducible, do not quote; see D-015):
  openramp_escrow.wasm  115750 B  sha256 a9de3590726ccb5f18be1bdfc5aa1f5c0e79699cab1cc1a245f4f38327782f26
  openramp_mock_ft.wasm 146163 B  sha256 253a471face17da507740f049d45cbbc246235fc462c01f76ee5d26ea42330e6
```
Failed / blocked: none.
Next: M1.2 LP registration, bond and liquidity deposits via `ft_transfer_call`, withdrawals.
Gotchas: commits no longer carry the Claude co-author / session trailers (Kwame, Nov 3).

## 2026-11-02 — Session 2: pawaPay decision + slash timing (docs only)
Branch: `docs/pawapay-decision-and-slash-rule` (from main after PR #3).
Done: D-011 (pawaPay sandbox for sell payouts, demo only; production = licensed LPs; Korapay
fallback; pawaPay deposits deferred). D-012 (failure classes LP_FAULT / OUTAGE; T_retry 24 h,
T_outage_max 72 h, demo 3 / 6 min; every failure still ends in a slash; reasoning recorded).
SPEC v0.3 (§4.5 classes table, §5 pawaPay specifics: UUID payout ids, canonical 233 numbers,
string amounts, status mapping; §7, inv. 11, §9), THREAT_MODEL T31, PLAN M0.6b (Test B2),
M1.6, M3.1/3.2, M4.6 (four demo outcomes incl. provider-down extension), M5.1; README;
.env.example. Checked: pawaPay sandbox needs no onboarding (how_to_start); prohibited list bans
"unlicensed … money transfer".
Failed / blocked: none. No code changed.
Next: [K] pawaPay sandbox token → Test B2. Meanwhile M1.1 escrow scaffold.

## 2026-11-01 — Session 2: M0.6 closed with decision D-010 (buy flow is the main demo)
Done: Kwame chose option 3. Recorded D-010; PLAN M0.6 ticked (charge PASS, transfers blocked),
M3.1/M3.2 use a `SimulatedPayout` provider in a separate, labelled verifier build, S1 dropped,
M5.1 README item added; SPEC §5 note, §9 and Q2 updated; AGENTS.md/CLAUDE.md agent-api line;
README status lines. No code changed.
Failed / blocked: none.
Next: M0.7 [K] fund mainnet dev account (~$3 USDC + a little NEAR) for Test C, or start M1
(escrow core) while waiting.
Gotchas: if Kwame's Paystack upgrade is approved later, rerun `scripts/test-b/run.py` (exit 0
expected) and switch M3 to real transfers.

## 2026-11-01 — Session 2: M0.6 Test B (BLOCKED: Paystack Starter account cannot send transfers)
Branch: `test-b/paystack-momo-payouts` (from main after PR #2).
Done: `scripts/test-b/run.py` (stdlib only; refuses unless Paystack says domain=test; masks phones):
```
PASS preflight: domain=test (ref opr-test-b-mode-1793571896)
PASS charge: HTTP 200, status=success, gateway=Approved, phone ***987
PASS charge verify: status=success, ref=opr-test-b-charge-1793571896, amount=1000 GHS, channel=mobile_money, paid_at=2026-11-01T22:24:57.000Z
PASS recipient: HTTP 201, RCP_ll1vjfsjqk79ot2, number ***987, bank MTN
BLOCKED transfer: HTTP 400, code=transfer_unavailable, message='You cannot initiate third party payouts as a starter business'
INFO transfer verify (no transfer): HTTP 404, code=not_found, message='Transfer not found'
exit 2
```
- GHS mobile money bank codes (GET /bank?currency=GHS&type=mobile_money): MTN, VOD (Vodafone/Telecel), ATL (AirtelTigo).
- Recipient object includes `details.account_number`, `bank_code` and `integration`: usable for the
  T23 recipient check, IF transfer verify returns the recipient (not yet seen: no transfer exists).
- Transfer verify "not found" is HTTP 404 `code: not_found` (payment verify uses 400
  `transaction_not_found`): the verifier must map each endpoint separately.
Failed / blocked: POST /transfer → 400 `transfer_unavailable`. Paystack help: Starter businesses
have no Transfers; upgrade to Registered Business on the Compliance page (needs business
registration docs and a matching business bank account).
https://support2.paystack.com/hc/en-us/articles/360009972719-What-is-a-Paystack-Starter-Business
Impact: buy flow (on-ramp) unaffected. Sell flow (M3) and agent-api stretch (S1) need payouts.
Next: Kwame decides (upgrade / other payout provider / sell flow with mocked payouts).

## 2026-11-01 — Session 2: M0.10 security design update (docs only)
Done: Reviewed Kwame's security audit (written before M0.4/M0.5). Already handled: #5 secret
access (M0.5), #9 callback (M0.4), #10 (the real verifier fails closed). Adopted #1, #2, #4, #6, #7,
#8 as SPEC v0.2 invariants 9–14 + THREAT_MODEL T23–T29; #3 adopted with a change (T30: every
failure still counts, because an LP can cause failures too). #11 → PLAN M2.6. #12: moved
`claude/agents/security-auditor.md` → `.claude/agents/` (merged main first: PR #1 + agent file).
Checked: Paystack verify response has no integration id (field list only), so key fingerprint it is.
Failed / blocked: none. No code changed.
Next: M0.6 Test B (transfers + transfer verify; also check recipient fields for T23).

## 2026-11-01 — Session 2: M0.5 DONE (Test A complete: real Paystack verdicts on OutLayer)
Done: Kwame stored the real test key as OutLayer secret profile `paystack` (wasm-hash 021f85…c61,
access allow-all via test-a.ikwame.testnet). All runs through `scripts/test-a/verify-run.sh`,
contract test-a.ikwame.testnet, results stored by the callback:
| Case | Reference | Stored verdict / reason | Tx |
|---|---|---|---|
| Real test MoMo payment | opr-test-a-paid-1 | PAID / ok | B69dGnWRqjZLGe2qn6ot5859t75jNQozdNn8rekcwWdF |
| Reference not found | opr-test-a-missing-1 | NOT_PAID / not_found | DAwdwkw24Y5Mrv5YBhUmxjyujxqWuP4dVrNpfADZHFWu |
| Abandoned (never paid) | opr-probe-modecheck-2 | NOT_PAID / not_success | 7B4Jy3Szjpm6YumSZGe6YuxmEtxrkbNxck48jkAbuRzD |
| Expected 2000, paid 1000 | opr-test-a-paid-1 | NOT_PAID / amount_short | An3aApfZ3xKKcwPM3cchdNozKRe2nXTFjyUAUCv9MmQh |
| Deadline before paid_at | opr-test-a-paid-1 | NOT_PAID / late | Bf1KiWeosu9HULw3cfGnAQ8jtarX79Q428ofpQj3SgbX |
| Invalid key (profile bad-key) | opr-test-a-paid-1 | UNVERIFIABLE / auth_error | 6ZzC8kmJtBXCJ3s4EbRGk4DoJJjaUqXuauh7jSc1wuMd |
| Real key called directly (not via contract) | — | refused: "Access denied by access condition" | vxWy9hD7zLdbPM6on1AR89kCXZ3Za4MrouXb4JQKPHn |
Test A (SPEC Q1) is answered: call shape, deposit (~0.003 NEAR estimate; 0.001 base fee kept on
failure), callback format, secret access, who can deliver the callback.
Failed / blocked: none.
Next: M0.6 Test B: Paystack test-mode mobile money transfer + transfer verify (the charge half
is already proven above). Then M0.7 [K] fund mainnet dev account, M0.8 Test C.
Gotchas: wasm outbound HTTPS works without any allowlist (no connector manifest). Paystack
`paid_at` is ISO 8601 with milliseconds and `Z`.

## 2026-11-01 — Session 2: M0.5 Test A part 3 (IN PROGRESS: waiting for Kwame to store the real key)
Done:
- Paystack key is TEST mode: `/transaction/initialize` (ref opr-probe-modecheck-2) then verify →
  `domain: test`, status `abandoned`. Custom `opr-…` references accepted.
- PAID fixture: `POST /charge` GHS 1000 pesewas, `mobile_money {phone 0551234987, provider mtn}`,
  reference `opr-test-a-paid-1` → `success` / `Approved`, channel mobile_money, paid_at
  2026-11-01T20:48:20Z (answers the charge half of SPEC Q2).
- `scripts/test-a/paystack-verify`: pure `decide()` + `reference_is_valid()` in lib.rs, wasi-http
  call in main.rs. `cargo test`: 15 passed. wasm sha256
  021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61, hosted in `wasm/` (commit 14ff01b).
- test-a contract: `request()` now takes optional `secrets_ref`; 6 tests pass; redeployed
  (tx 5GETLALTzZK7j1o3MLc4AjFSKMPGBVHY3sVybvmCV25G).
- `scripts/test-a/verify-run.sh <profile> <reference>` runs one check through the contract.
- UNVERIFIABLE (revoked/invalid key): secret profile `bad-key` = `sk_test_000…` (not a real key),
  bound to the wasm hash, access `allow-all --via test-a.ikwame.testnet` →
  `verify-run.sh bad-key opr-test-a-paid-1` → stored
  `{"verdict":"UNVERIFIABLE","reason":"auth_error"}` (tx 6ZzC8kmJtBXCJ3s4EbRGk4DoJJjaUqXuauh7jSc1wuMd).
- Access control works: same secret called directly by ikwame.testnet (not via the contract) →
  "Access denied by access condition — signer ikwame.testnet, called from ikwame.testnet"
  (tx BLA7e4tvgUPgWuHxCgVccHLtdRmuinHfx9SuGMh4P8z4).
Failed: first try used `--access whitelist:test-a… --direct` → denied, because a Whitelist is
checked against the SIGNER (ikwame.testnet). The right rule is `--access allow-all --via <contract>`
= `Predecessor{Whitelist[contract]}`: any signer, but only through that contract. For the escrow
this is what each LP stores (plus the build lock).
Next: [K] store the real test key as profile `paystack` with the same binding/access, then run
PAID (`opr-test-a-paid-1`) and NOT_PAID (`opr-test-a-missing-1`, and `opr-probe-modecheck-2` = abandoned).
Gotchas: `outlayer secrets list` shows nothing for wasm-hash rows. Every verifier rebuild changes
the hash, so the secret must be re-bound (`outlayer secrets access --build`) or re-stored.

## 2026-11-01 — Session 2: M0.4 DONE (contract → OutLayer → callback)
Done:
- `scripts/test-a/contract` (near-sdk 5.30): `request(wasm_url, wasm_hash, input)` calls
  `outlayer.testnet.request_execution` (200 TGas, deposit = attached) `.then(on_result)` (30 TGas,
  `#[private]`); `on_result` sorts the result with `classify()` and stores it; `get_records` view.
  `cargo test`: 6 passed (output / null / empty / not-JSON / too long / promise failed).
  `cargo near build non-reproducible-wasm` OK. Deployed to test-a.ikwame.testnet (3 NEAR,
  create tx Dd4nhjpQ1QS3K6amK6yNqq47z7pUmZ4PS1BdCkUmKv6U, deploy tx 3y9YaotkF3gbYS4batLRueKQghFhT7AZYcmXzaPhdkdH).
- Run 1, real hello.wasm → stored `{"kind":"Output","value":{"input":{"command":"hello","order":"opr-7"},
  "message":"Hello, ikwame.testnet!","success":true}}` (block 275800166). Attached 0.01 NEAR;
  estimate_execution_cost said 0.0030001 NEAR.
- Run 2, wrong hash (00…00) → OutLayer `"wasm_url content does not match its hash"`, charged
  0.001 NEAR, refunded 0.009 → stored `{"kind":"RunFailed"}` (tx CD1r8t2AmjYsX7Keyt1pRfRvWrJfcSidsxgWvEHoMSMW).
- Run 3, ikwame.testnet calls `on_result` directly → `Smart contract panicked: Method on_result is
  private`; no record added (T13 confirmed).
- `near view test-a.ikwame.testnet get_records` → exactly the 2 records above.
- SPEC Q1 answers: call shape and deposit as above; our contract owns the `.then` callback,
  OutLayer owns the yield (it resumes its own `on_execution_response`, whose return value is what
  our callback gets). Result = program JSON on success, `null` on failed run, empty on promise
  failure / timeout. Only the OutLayer operator can resolve a request (`assert_operator`).
  `NEAR_SENDER_ID` in the wasm = the transaction signer (ikwame.testnet), not the calling contract.
  Source: github.com/out-layer/outlayer contract/src/execution.rs, app.outlayer.ai/docs/near-integration.
Findings for the escrow (added to THREAT_MODEL): T21: the escrow must fix the verifier wasm
URL+hash itself, never take it from the caller. T22: operator trust, same as T17.
Failed / blocked: none. Timeout path not triggered live (can't force OutLayer to stay silent);
covered by reading the source + unit test.
Next: M0.5 Test A part 3: wasm calls Paystack with the key from an OutLayer secret; PAID /
NOT_PAID / UNVERIFIABLE. Needs Kwame to store the Paystack test key as an OutLayer secret.
Gotchas: `PromiseError` is non_exhaustive (needs a catch-all arm; fail closed). Use
`promise_result_checked` (old `promise_result` is deprecated). near CLI echoes the private key
in its "re-run" hint: always filter output.

## 2026-11-01 — Session 2: M0.3 DONE (hello-world OutLayer run from the CLI)
Done:
- Repo is already PUBLIC (Kwame checked Settings; earlier notes saying "private" are outdated).
- Hosted the wasm in `wasm/hello.wasm` (D-008). Raw URL pinned to commit 2b53680, downloaded sha256
  matches 0ca1af08…a36b.
- `outlayer run --wasm <raw url> --hash 0ca1af08… --input in.json` → tx
  Gos57Sqjrf4RX3vS8KbRWQe2PyiDgLZDn6RkbupkWL5J, output:
  ```
  {"input": {"command": "hello", "order": "opr-1"}, "message": "Hello, ikwame.testnet!", "success": true}
  ```
  Echo verified: the input comes back unchanged, through OutLayer's on-chain callback.
Failed / blocked:
- `outlayer deploy hello <raw url>` → "Version '0ca1af08…' already exists in project": versions
  are keyed by hash, and that hash is still bound to the dead FastFS URL. Used `--wasm` instead.
- With `--wasm`, a positional JSON arg is read as PROJECT, not INPUT (got `"input": {}`). Use
  `--input <file>`.
- test.fastfs.io still 502 at end of session.
Next: M0.4 Test A part 2: tiny testnet contract calls `request_execution` and stores the callback
result (call shape already recorded in the M0.3 part 1 entry below).
Gotchas: `NEAR_SENDER_ID` inside the wasm = the account that signed the request (here
ikwame.testnet); in M0.4 check whether it becomes the calling contract.

## 2026-11-01 — Session 2: M0.3 continued (BLOCKED: FastFS testnet gateway down)
Done:
- Funding: near-faucet.io failed for Kwame ("There was an error", 0 NEAR arrived). Workaround:
  `near account create-account sponsor-by-faucet-service openramp-funder-64532.testnet
  use-manually-provided-public-key <ikwame.testnet public key>` (got 10 NEAR, same key, no new
  secret), then sent 9.9 NEAR → ikwame.testnet. Balance now 9.9 NEAR.
- `outlayer upload out/hello.wasm` OK →
  https://test.fastfs.io/ikwame.testnet/outlayer.testnet/0ca1af08790c4d29256766cb7193840c908339e3353221e397418814cdfaa36b.wasm
- `outlayer deploy hello <url> --hash 0ca1af08…a36b` → "Project deployed" (without `--hash` the CLI
  tries to download the file and fails on the 502).
- `outlayer run ikwame.testnet/hello '{"command":"hello"}'` → tx
  BVDybc2yosHq9ZNUHRTJiCfbEyjvbknBjUzsXDiMWdna. The on-chain round trip WORKS:
  `execution_requested` (request_id 6435) → yield → `execution_completed` callback on
  outlayer.testnet, charged 0.001 NEAR then refunded. Final tx value: `null`.
Failed / blocked:
- Worker log: `Execution failed: "wasm_url could not be fetched"`. `curl https://test.fastfs.io/`
  (gateway root, not just our file) → `502 error code: 502` (Cloudflare). FastFS testnet is down.
  Attempt 1: deploy without --hash (502). Attempt 2: deploy with --hash + run (worker can't fetch).
- Docs (https://app.outlayer.ai/docs/getting-started): "Any https URL works for `outlayer deploy`";
  the worker checks the sha256. So a public HTTPS host for the wasm is a valid fallback.
Next (Kwame to choose): (a) retry later when test.fastfs.io is back, or (b) host hello.wasm at a
public HTTPS URL (our repo is private, so raw.githubusercontent won't work without making it
public; e.g. a release asset in a small public repo).
Gotchas: an execution failure is still delivered through the callback (with the fee refunded),
so the escrow must treat an OutLayer error as UNVERIFIABLE (SPEC §4.5 already says so). Set
RUST_BACKTRACE=0 to get readable outlayer CLI errors.

## 2026-11-01 — Session 2: M0.3 Test A part 1 (BLOCKED: testnet account has 0 NEAR)
Done:
- `outlayer login testnet` with NEAR_ACCOUNT_ID / NEAR_PRIVATE_KEY piped from env (key saved to
  ~/.outlayer, outside the repo). `outlayer whoami` → ikwame.testnet, contract outlayer.testnet.
- `outlayer create hello` → `scripts/test-a/hello` (template already echoes the JSON input).
  `./build.sh` → `out/hello.wasm`, 173717 bytes,
  sha256 0ca1af08790c4d29256766cb7193840c908339e3353221e397418814cdfaa36b.
- From outlayer-cli 0.2.1 source (`src/commands/run.rs`), the on-chain call (useful for M0.4):
  `outlayer.testnet.request_execution({source: {Project:{project_id,version_key}} |
  {WasmUrl:{url,hash,build_target}}, resource_limits:{max_instructions,max_memory_mb,
  max_execution_seconds}, input_data:<JSON string>, secrets_ref:{profile,account_id}|null,
  response_format:"Json", payer_account_id:null, params:null})`, 300 TGas, deposit =
  `estimate_execution_cost(resource_limits, input_len)` view call (yoctoNEAR).
  Without PAYMENT_KEY, `outlayer run` uses this on-chain path.
Failed / blocked:
- RPC view_account ikwame.testnet: `amount 0`, 1 full-access key (matches env key). Upload
  (FastFS tx), deploy and run all need testnet NEAR. [K] fund the account, then rerun:
  `outlayer upload out/hello.wasm` → `outlayer deploy hello <url>` →
  `outlayer run ikwame.testnet/hello '{"command":"hello"}'`.
Next: after funding, finish M0.3 (paste run output), then M0.4.
Gotchas: `outlayer` commands all need `outlayer login` first (even `create`). The OutLayer
getting-started page says "Live on NEAR Mainnet" but the CLI supports testnet (outlayer.testnet).

## 2026-11-01 — Session 2: M0.2 toolchain
Done: toolchain installed from crates.io (cargo install --locked), all checks pass:
```
rustc 1.97.0 (2d8144b78 2026-07-07)
targets: wasm32-unknown-unknown, wasm32-wasip2, x86_64-unknown-linux-gnu
cargo-near-near 0.22.0
near-cli-rs 0.30.1
outlayer 0.2.1   (outlayer --help OK)
```
Recipe that works in the cloud container:
```
rustup target add wasm32-unknown-unknown wasm32-wasip2
cargo install --locked outlayer-cli
cargo install --locked cargo-near --no-default-features
cargo install --locked near-cli-rs --no-default-features --features inspect_contract,verify_contract
```
Failed / blocked:
- OutLayer's official `install.sh` (https://outlayer.ai/docs/getting-started) was blocked by the
  auto-mode safety check; the crates.io `outlayer-cli` points to the same repo (out-layer/outlayer-cli).
- api.github.com returned 403 through the proxy, so GitHub release binaries were not used.
- First cargo-near build failed: default `ledger` feature needs system `libudev` (not installed).
  Ledger is not needed, so built without default features.
Next: M0.3 Test A part 1 (hello-world OutLayer WASM). Needs `outlayer login` with the testnet key.
Gotchas: containers start clean; all of the above is lost. Full build ~10 min. See PLAN M0.2b.

## 2026-11-01 — Session 2: secrets check (handoff task 1)
Done:
- Env vars (checked set/missing only, values never printed):
  `PAYSTACK_SECRET_KEY missing`, `NEAR_ACCOUNT_ID set` (a `.testnet` account),
  `NEAR_PRIVATE_KEY set` (`ed25519:` format), `ONECLICK_API_KEY missing`.
- Paystack key is injected by the agent proxy, not an env var:
  `GET /transaction/verify/opr-probe-does-not-exist` → HTTP 400 `transaction_not_found`
  (would be 401 without auth); `GET /transaction?perPage=1` → `status True`, 0 transactions.
- Documented both mechanisms in `.env.example`.
Failed / blocked: none.
Open: not yet proven that the injected key is a TEST key (no field says so; account has no
transactions). Confirm in M0.6 by checking `domain == "test"` on the first charge.
M0.1 still open: mainnet dev account + 1Click key (for M0.7–M0.8) [K].
Next: M0.2 toolchain from scratch (cargo-near, near-cli-rs, outlayer CLI), then M0.3.
Gotchas: Paystack code must not fail when PAYSTACK_SECRET_KEY is empty in cloud sessions (proxy
adds auth). The OutLayer TEE will NOT have this proxy, so the verifier still needs the key as an
OutLayer secret (M0.5).

## 2026-11-01 — Handoff: end of session 1 (switching to a new session so secrets load)
Current state: Docs only (AGENTS, SPEC, THREAT_MODEL, PLAN, DECISIONS, PROMPTS). Kwame APPROVED
SPEC + PLAN. No code yet.
M0.1 status: Kwame created a Paystack test account and a NEAR testnet account (Meteor Wallet) and
added the keys to the cloud environment as env vars / Network secrets. Not yet done: NEAR mainnet
dev account (~$3 USDC + a little NEAR) and 1Click API key (needed only for Test C, M0.7–M0.8).
Secrets were NOT visible in session 1 (container started before they were added).
In progress: M0.2 toolchain. In session 1 only `rustup target add wasm32-unknown-unknown
wasm32-wasip2` ran; containers start clean, so redo M0.2 from scratch.
Next 3 tasks:
1. Check which secrets are visible without printing values:
   `for v in PAYSTACK_SECRET_KEY NEAR_ACCOUNT_ID NEAR_PRIVATE_KEY; do [ -n "${!v}" ] && echo "$v set" || echo "$v missing"; done`
   If missing, they may be Network secrets injected by the proxy into requests instead of env vars:
   probe with `curl -sS https://api.paystack.co/bank?country=ghana -o /dev/null -w '%{http_code}'`
   (200 = auth injected, 401 = not). Record which mechanism works in PROGRESS.md and .env.example.
2. M0.2 toolchain: wasm targets, cargo-near, near-cli-rs, outlayer CLI (install steps:
   https://outlayer.ai/docs/getting-started). Check versions.
3. M0.3 Test A part 1: hello-world OutLayer WASM run from the CLI.
Gotchas: Rust 1.97 and Node 22 are preinstalled. Outbound HTTPS goes through a proxy (CA bundle
/root/.ccr/ca-bundle.crt). Kwame prefers plain, short explanations; he got lost once, so state
next steps simply.

## 2026-11-01 — Session 1: project setup (kickoff)
Done:
- Repo created (`Ikwame91/near-momo-p2p`, private). Name chosen: OpenRamp (D-005).
- Wrote AGENTS.md + CLAUDE.md (copy), docs/SPEC.md, THREAT_MODEL.md, PLAN.md, DECISIONS.md,
  PROMPTS.md, .gitignore, .env.example, LICENSE (MIT, required by hackathon rules).
- Checked HACKATHON.md: repo must be public by Nov 14 23:59 UTC, LICENSE required, README with
  "do X → see Y", X post with video, Intents is mainnet-only, AI judges (Claude Code, Codex) screen
  reproducibility.
- OutLayer docs (https://app.outlayer.ai/docs): call `request_execution` on `outlayer.testnet`,
  result via yield/resume callback, secrets injected as env vars, build `wasm32-wasip2`.
Failed / blocked: none. Feasibility tests A, B, C have NOT been run yet.
Next: Kwame approves SPEC + PLAN → M0.1 accounts [K] and M0.2 toolchain → M0.3 Test A.
Gotchas: mainnet OutLayer account id not on the docs index page; full `request_execution` schema is
on https://outlayer.ai/docs/near-integration.
