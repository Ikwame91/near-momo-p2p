# Agent Prompt Kit: OpenRamp

> Section 1 is the original template. The live rules are in `AGENTS.md` (and its copy `CLAUDE.md`);
> if they differ, `AGENTS.md` wins.

The core idea: **an agent forgets; files don't.** Long context, compaction, and restarts wipe the agent's memory, so every important fact, decision and next step must live in files in the repo that the agent re-reads at the start of every session. Prompts tell it *how to work*; files tell it *where it is*.

Set up these files in the repo root on day 1:

| File | Purpose | Who updates it |
|---|---|---|
| `AGENTS.md` (plus `CLAUDE.md` as an exact copy, not a symlink) | Permanent rules: mission, architecture, conventions, guardrails | You, rarely |
| `docs/SPEC.md` | What the product does, flows, invariants, out of scope | You + agent, on decision |
| `docs/PLAN.md` | Milestones broken into small tasks with checkboxes | Agent, every task |
| `docs/PROGRESS.md` | Running log: what was done, what failed, what's next | Agent, end of every session |
| `docs/DECISIONS.md` | Architecture decision records (why X over Y) | Agent, on every decision |
| `docs/THREAT_MODEL.md` | Attacks and the defence for each | You + agent |
| `README.md` | Written for judges, including AI judges: problem, how to run, "do X, see Y" | Agent, near the end |

---

## 1. `AGENTS.md`: the permanent rules file

Paste this, then edit the bracketed bits.

```markdown
# AGENTS.md: read this fully before doing anything

## Mission
We are building OpenRamp: a trustless peer-to-peer exchange between Ghana mobile money
(MTN MoMo, Telecel Cash, AirtelTigo Money) and stablecoins (USDC) on NEAR, for the NEAR Hackathon
(build Nov 1–14, 2026; submit by Nov 14 23:59 UTC). Tracks: Real-World Money (main),
Agent Economy (secondary). Privacy only if we actually build something for it.
Owner: Kwame (all modules) for now.

## Architecture (do not change without a DECISIONS.md entry)
- `contracts/escrow`: NEAR smart contract (Rust, near-sdk). Holds USDC, manages orders, LP bonds,
  timeouts, bond slashing. Uses yield/resume to wait for the OutLayer verification result.
- `verifier/`: OutLayer program. Checks a mobile money payment via the payment provider API
  (Paystack TEST MODE only) inside the TEE. The verdict returns to the escrow contract through
  OutLayer's yield/resume callback; there is no separate signature to check unless OutLayer's docs
  show one (confirm in Test A). Provider is a plug-in
  behind an interface, so it can be swapped for a licensed partner.
- `settlement/`: NEAR Intents (1Click API) integration for cross-chain USDC in/out.
- `agent-api/` (STRETCH, only after the buy flow works end to end): x402 endpoint so AI agents
  can pay a Ghanaian phone number in cedis. Needs the full sell flow plus Paystack payouts.
- `web/`: minimal frontend for buyer, seller and LP flows.

## Session protocol (EVERY session, no exceptions)
1. Read AGENTS.md, docs/SPEC.md, docs/PLAN.md, and the last 3 entries of docs/PROGRESS.md.
2. State in 3 lines: where we are, the single task you will do now, how you will verify it.
3. Do ONLY that task. If you discover other work, add it to PLAN.md; don't do it now.
4. Verify (tests/build/run) before claiming done. Paste the actual command output.
5. Update PLAN.md checkboxes and append a PROGRESS.md entry (done / failed / next).
6. Commit with a clear message. One logical change per commit.

## Engineering rules
- Small steps: change <~200 lines at a time, then test. Never rewrite a working module wholesale.
- Tests first for contract logic: write the failing test, then the code. Every invariant in SPEC.md
  has a test.
- Never mark a task done without running it. "Should work" is not done.
- Don't guess APIs. Read the docs or the SDK source, or write a tiny probe script. Cite the doc URL
  in a code comment for any external API call.
- Keep it boring: simple, explicit code over clever abstractions. No new dependency without a reason
  in DECISIONS.md.
- Errors are explicit: no silent catches; failing closed is the default for anything touching money.
- Config and secrets only via env vars. Never commit keys. `.env.example` lists every variable.
- Testnet and Paystack test mode by default. Mainnet only with an explicit human instruction and
  tiny amounts.
- If stuck after 2 attempts on the same error: stop, write what you tried in PROGRESS.md, and ask.

## Money and security invariants (must always hold)
- USDC is released only on a PAID verdict that arrives through the OutLayer callback for THIS
  order ID. Verdicts from any other caller or for any other order are rejected.
- "Could not verify" is NEVER treated as "not paid". After a retry window for real outages, a
  still-UNVERIFIABLE result slashes the LP's bond to the buyer. There is no dispute path and no
  admin or arbiter; adding one breaks the trustless claim.
- A payment reference can be used for exactly one order (replay protection).
- An LP cannot withdraw their bond or change their registered provider key while any order is open.
- If an LP revokes the key on Paystack's side, the check comes back UNVERIFIABLE and the bond is
  slashed under the rule above. The bond is the real defence against key revocation; never skip it.
- New-buyer amount cap and release delay are configurable settings (0 delay in the demo) to
  protect LPs from MoMo reversals; the README explains them.
- No phone numbers or API keys on-chain or in logs.

## Definition of done (per task)
Code + tests passing + PLAN.md ticked + PROGRESS.md entry + commit.

## Never
- Never delete or rewrite tests to make them pass.
- Never change the architecture, invariants or scope silently.
- Never touch mainnet, real money, or real user data without an explicit human go-ahead.
- Never post or submit anything. Push freely; ask before making the repo public or submitting.
```

---

## 2. Kickoff prompt (session 1 only)

```text
Read AGENTS.md fully. Then, before writing any code:
1. Write docs/SPEC.md: the buy flow (cedis → USDC) and sell flow (USDC → cedis) step by step,
   the actors (buyer, seller, LP, verifier, contract), every state an order can be in and the
   allowed transitions, the invariants from AGENTS.md, and an explicit OUT OF SCOPE list.
2. Write docs/THREAT_MODEL.md: list every way a buyer, seller, LP or outsider could cheat
   (fake receipt, replayed reference, key revocation on Paystack's side, MoMo reversal, timeout
   griefing, front-running) and the defence for each. Defences may not rely on an admin or arbiter.
3. Write docs/PLAN.md: milestones M0–M5, each broken into tasks of ≤2 hours, each task with a
   verification step. M0 = the three feasibility tests, run right after the docs are approved, starting
   with Test A (contract↔OutLayer↔Paystack test mode, the riskiest unknown), then B (Paystack test
   MoMo charge + transfer) and C ($1 NEAR Intents withdrawal to Base).
4. Create docs/PROGRESS.md and docs/DECISIONS.md with templates.
Do not write application code in this session. Show me the plan and wait for approval.
```

---

## 3. Start-of-session prompt (paste at the start of every new chat/session)

```text
New session. Follow the session protocol in AGENTS.md exactly:
read AGENTS.md, docs/SPEC.md, docs/PLAN.md and the last 3 PROGRESS.md entries.
Then tell me in 3 lines: current milestone, the next unchecked task, and how you'll verify it.
Wait for my "go" before coding.
```

---

## 4. Task prompt (for each piece of work)

```text
Task: [e.g. "M2.3 Add replay protection: a payment reference can only settle one order"].
Context: see SPEC.md §[x] and THREAT_MODEL.md item [y].
Steps:
1. Write a failing test that proves the bug/feature (show me it failing).
2. Implement the smallest change that makes it pass.
3. Run the full test suite and show the output.
4. Update PLAN.md, append to PROGRESS.md, commit.
Constraints: touch only [files/folders]. Do not change other modules. If the spec is ambiguous,
stop and ask instead of guessing.
```

---

## 5. Debugging prompt

```text
This is failing: [paste error + command].
Don't change code yet. First:
1. Reproduce it with the smallest command possible.
2. List 3 likely causes, ranked, with evidence for each.
3. Add logging/a probe to confirm the top cause.
Only then fix it, with a regression test. If 2 fixes fail, stop and write up what you learned
in PROGRESS.md.
```

---

## 6. Checkpoint / anti-drift prompt (every ~2 hours or when it feels lost)

```text
Pause. Re-read AGENTS.md and docs/PLAN.md.
1. Summarize what you changed since the last checkpoint (files + why).
2. Check every invariant in AGENTS.md still holds; run the full test suite.
3. List anything you did that is NOT in PLAN.md (scope creep) and whether to keep or revert it.
4. Update PROGRESS.md with a checkpoint entry. Then tell me the next task.
```

---

## 7. Context-reset handoff prompt (before the context gets long, ~60–70% full, or end of day)

```text
We're about to start a fresh session. Write a handoff entry at the top of docs/PROGRESS.md:
- Current state: what works end-to-end right now (with the command to see it)
- In progress: the exact task, files touched, what's left
- Known bugs / failing tests
- Decisions made today (also add to DECISIONS.md)
- Next 3 tasks in order
- Gotchas the next session must know (API quirks, env vars, commands)
Make it complete enough that a new agent with zero memory can continue. Commit it.
```

---

## 8. Code review prompt (before merging anything touching money)

```text
Act as a hostile security reviewer of the escrow contract and verifier. For the diff since
[commit], check: reentrancy and callback failure handling on yield/resume, integer/decimal
handling for USDC (6 decimals), access control on every public method (especially that verdicts come only from the OutLayer
callback for that order), replay of payment refs, timeout, retry window and bond-slashing paths, what happens if OutLayer returns an error or never responds,
storage staking, and any secrets in logs. Output a list of findings by severity with the exact
line, and a test that proves each one. Do not fix yet.
```

---

## 9. Pre-submission prompt (Nov 12–13)

```text
Prepare for judging. Judges include investors and AI judges that read the README.
1. Rewrite README.md for a machine reader: what it is, the problem (with sources),
   architecture diagram (mermaid), how to run locally in ≤5 commands, "do X, see Y" demo steps,
   what's real vs test mode (Paystack test mode, testnet), the buyer cap and delay settings
   (0 in the demo), tracks (Real-World Money main, Agent Economy secondary), and limitations. No marketing.
2. Run a fresh clone → setup → demo from scratch and fix anything that breaks.
3. Make sure no secrets are in the repo history.
4. Write a 2-minute demo script matching the README steps.
```

---

## 10. Engineering patterns worth naming to the agent

- **Plan → act → verify → record** loop, every task.
- **Single source of truth:** the files above, not chat memory.
- **Small diffs, frequent commits:** easy to review, easy to revert (`git revert`, not panic rewrites).
- **Test-driven for invariants:** money rules are tests, not comments.
- **Fail closed:** never auto-release on doubt; after the retry window, UNVERIFIABLE slashes the
  LP bond to the buyer.
- **Idempotency:** every external call (Paystack, Intents) is safe to retry, keyed by order ID.
- **Ports and adapters:** payment provider behind an interface, so test mode, Paystack or a
  licensed partner can be swapped.
- **Architecture decision records:** a short why for every big choice, so later sessions don't
  undo it.
- **Feature freeze:** stop adding features on Nov 12; only fixes, README and demo after that.
- **Human gates:** you approve the spec, the plan, anything touching mainnet, and the submission.

## Practical tips
- Use one agent session per milestone; start fresh rather than dragging a huge context along.
- If the agent starts contradicting earlier decisions or re-doing finished work, it's lost: run
  prompt 7, then start a new session with prompt 3.
- Kwame owns every module for now. If Jeffery joins later, update the owner line in AGENTS.md
  and CLAUDE.md so the agent knows whose code it's touching.
