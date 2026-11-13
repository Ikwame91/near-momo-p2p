# UI Skills for OpenRamp `web/`

Researched Nov 13, 2026. Star counts come from GitHub search on that date.
Goal: make the buyer, seller and LP screens feel trustworthy and polished without slowing the build.

## Recommended set (install these, nothing more)

Three skills, each with its own job. Stacking every popular design skill gives Claude conflicting
instructions and makes the output worse, so pick from this list on purpose.

| # | Job | Skill | Source | Why |
|---|---|---|---|---|
| 1 | Look and feel | `frontend-design` | [anthropics/skills](https://github.com/anthropics/skills) (Anthropic, ~194k★) | Makes Claude pick a deliberate visual direction (type, colour, layout) before coding. Avoids the generic "AI app" look (Inter, purple gradients, card soup). |
| 2 | Quality check | `web-design-guidelines` | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) (Vercel, ~34k★) | Checks UI code against 100+ rules: accessibility, focus states, contrast, form labels, loading/error states, mobile tap targets, performance. |
| 3 | React performance (only if `web/` is React/Next.js) | `react-best-practices` | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) | Performance rules ranked by impact: re-renders, data fetching, bundle size. Matters for live order-status screens. |

Component library (not a skill): **[shadcn/ui](https://ui.shadcn.com)**. Its forms, dialogs and
toasts are accessible out of the box.

### Install

```bash
npx skills add anthropics/skills --skill frontend-design --agent claude-code
```

```bash
npx skills add vercel-labs/agent-skills --skill web-design-guidelines --agent claude-code
```

```bash
npx skills add vercel-labs/agent-skills --skill react-best-practices --agent claude-code
```

Skills end up in `.claude/skills/<name>/SKILL.md`. Commit them so every session gets the same rules.
Per AGENTS.md ("no new dependency without a reason"), add a DECISIONS.md entry when you install them.

## How to use them

1. **Before building a screen:** ask Claude to use `frontend-design` to set the visual direction
   once (fonts, colour tokens, spacing scale, tone). Save the result as design tokens (CSS
   variables / Tailwind config) so every screen reuses them.
2. **While building:** use shadcn/ui components; follow `react-best-practices` for data fetching
   and state.
3. **Before each commit / demo:** run a review with `web-design-guidelines` on the changed files and
   fix what it finds.

## What "feels good" means for OpenRamp specifically

These matter more than any visual style, because users are trusting the app with money:

- **Every order state is visible and plain-spoken:** Awaiting payment → Verifying → Released, plus
  Unverified / Expired / Slashed / Refunded. Each one says what is happening and what happens next.
- **Countdowns** for `pay_deadline` and `T_retry`, so the buyer knows how long they have.
- **Exact amounts:** cedis to pay, USDC to receive, rate, fees. Shown before confirming, never rounded
  silently.
- **Copy buttons** for the payment reference and amounts; a confirmation toast when copied.
- **Loading, empty and error states** on every screen; errors say what to do, not just "failed".
- **Mobile first:** most users will be on a phone paying with MoMo. Big tap targets, no horizontal
  scroll.
- **Never show or store phone numbers or keys** in the UI state, URLs or logs (invariant 7).
- **Links to the NEAR explorer** for every on-chain transaction, so users can check for themselves.

## Alternatives (considered, not chosen)

| Skill | Source | Why not now |
|---|---|---|
| UI UX Pro Max | [nextlevelbuilder/ui-ux-pro-max-skill](https://github.com/nextlevelbuilder/ui-ux-pro-max-skill) (~149k★, MIT) | Big catalogue of styles, palettes, font pairings and UX rules, searched through Python scripts. Overlaps with `frontend-design`; use it *instead* of #1 if you want ready-made palettes, not as well as it. |
| Taste Skill | [Leonxlnx/taste-skill](https://github.com/Leonxlnx/taste-skill) (~115k★, MIT) | Same goal as `frontend-design` (opinionated, anti-generic taste). Pick one of the two. |
| Interface Design | [Dammyjay93/interface-design](https://github.com/Dammyjay93/interface-design) (~6k★, MIT) | Remembers your design system across sessions. Useful later; last updated June 2026. |
| webapp-testing | [anthropics/skills](https://github.com/anthropics/skills) | Lets Claude drive the app in a browser to check it renders and works. Good addition once `web/` exists. |

## Safety note

A skill is instructions plus scripts that Claude follows and runs inside the repo. The Anthropic and
Vercel skills are low risk. For third-party skills, read the `SKILL.md` and every script before
installing, and pin to a commit.

## Sources

- [9 Best UI Skills for Claude Code in 2026 (Medium)](https://medium.com/@hii_mohit/9-best-ui-skills-for-claude-code-in-2026-i-tested-them-all-58fe42809376)
- [14 Best Claude Code Skills for Developers (Firecrawl)](https://www.firecrawl.dev/blog/best-claude-code-skills)
- [20 Best Claude Code Skills for UI/UX Design](https://pasqualepillitteri.it/en/news/576/claude-code-skills-design-uiux-guide)
- [Top 8 Claude Skills for UI/UX Engineers (Snyk)](https://snyk.io/articles/top-claude-skills-ui-ux-engineers/)
- [Best Claude Code Skills 2026 (DesignRevision)](https://designrevision.com/blog/best-claude-code-skills)
- [Best UI/UX Skills for Claude Code (claudeskills.info)](https://claudeskills.info/best/ui-ux-skills/)
- [UI UX Pro Max on skills.sh](https://skills.sh/nextlevelbuilder/ui-ux-pro-max-skill/ui-ux-pro-max)
- [frontend-design on playbooks.com](https://playbooks.com/skills/anthropics/skills/frontend-design)
- [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills)
