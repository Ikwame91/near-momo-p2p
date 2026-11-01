# Progress log

Newest entry at the top. Template:
```
## YYYY-MM-DD HH:MM UTC — <session / task>
Done:
Failed / blocked:
Next:
Gotchas:
```

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
