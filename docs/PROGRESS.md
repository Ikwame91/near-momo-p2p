# Progress log

Newest entry at the top. Template:
```
## YYYY-MM-DD HH:MM UTC — <session / task>
Done:
Failed / blocked:
Next:
Gotchas:
```

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
