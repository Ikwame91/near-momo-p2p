# UI review: Web Interface Guidelines (Nov 13, 2026)

Skill: `.claude/skills/web-design-guidelines` (vercel-labs/agent-skills at commit
`1a2c79d4e6c4a2283dc2c614b6538cbe1ed5ca62`), rules from
https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md (fetched Nov 13).
Files: `web/index.html`, `web/src/*.js`, `web/src/style.css`. Decision: DECISIONS D-028.

## Fixed

- `web/src/style.css` – no `touch-action: manipulation` / tap highlight → added on buttons, links, inputs
- `web/src/style.css` – buttons, chips, links had no hover state → added under `@media (hover: hover)`
- `web/index.html` – no skip link → "Skip to buying USDC" (visible on focus)
- `web/index.html` – one `theme-color` for both themes → light and dark values
- `web/index.html` – brand could be machine-translated → `translate="no"`
- `web/src/main.js` – amount input lacked `name` and `spellcheck=false` → added
- `web/src/main.js` – copied values gave no feedback (no copy at all) → Copy buttons for amount and
  reference, with a toast

## Gaps from the "feels good" list (Kwame's research), fixed

- Status pills said "Pending" for four different states → Awaiting payment, Check due, Verifying,
  Not confirmed, Releasing; "Slashed" → "Bond paid" (`statusPill`, tested)
- No countdown while a check is "Not confirmed" → live "Bond window" clock, mirroring the
  contract's `due_at` (`bondPaysAt`, tested)
- NEAR fees not shown before confirming → exact amounts under the Open order button (`fmtNear`, tested)
- No copy buttons on the pay screen → amount and order reference

## Checked, already fine

Icon-only buttons have `aria-label`; toasts use `role="status"`/`"alert"` (live regions); errors
say what to do; `prefers-reduced-motion` respected; spinners on every wallet step; no
`transition: all`; focus visible (`:focus-visible`, amount uses `:focus-within`); `color-scheme`
set; explorer links on every order; no phone numbers or keys in the page.

## Declined, on purpose

- Title Case for headings and buttons: the page uses sentence case throughout (docs/DESIGN.md copy rule).
- `Intl.NumberFormat` for money: amounts are exact BigInt formatting that matches the contract and is
  tested; a locale formatter could round.
- Focused order in the URL: `?view=` covers sharing; per-order deep links are not needed for the demo.
