# Web page design: "wax print"

The buyer and seller pages (`web/`) follow one direction, chosen on Nov 13, 2026 from three
rendered options (a forex rate board, a wax-print cloth, a sealed envelope) using the App Designer
process. Decision: DECISIONS D-027.

## Concept

**OpenRamp is a wax-print cloth, stamped one adinkra symbol per step.** The page is a cobalt
colour field, like a wax-print ground. The progress band at the top replaces the numbered step
tracker: Dwennimmen (ram's horns: strength with humility) and Nkyinkyim (the twisting path)
alternate, and each step "stamps" its symbol in white. The headline borrows the sealed-envelope
idea: "Buy USDC. It stays **sealed** until Paystack sees your MoMo."

Refused defaults: money green (the category's colour), dark-fintech lime on black, Ghana flag
colours, MTN yellow, Inter, cards inside cards, an icon-tile "How it works" grid.

## Tokens (`web/src/style.css`)

| Role | Light (cobalt) | Dark (night) |
|---|---|---|
| ground `--bg` | `#2340b0` | `#0e1a47` |
| raised `--card` (chips, inputs, toasts, pills) | `#18308a` | `#172763` |
| ink `--ink` | `#ffffff` | `#f2f5ff` |
| ink-2 `--ink-2` / `--muted` | `#d3dcff` | `#afbbea` |
| rule `--line` (also unstamped symbols) | `#4762c9` | `#2b3c7d` |
| accent `--brand`: the primary action only | `#ff5b3a` | `#ff6e50` |
| on-accent `--brand-ink` (dark text: white on tomato fails contrast) | `#1c0904` | `#1c0904` |
| status: pending / ok / bad | `#ffd27a` / `#a6f0c8` / `#ffbcae` | `#f7c766` / `#8fe3b7` / `#ffaa9a` |

Type: Bricolage Grotesque (OFL, Google Fonts), one family. Display moments: the amount input at
96px/800 with -5% tracking, the received amount at 72px, screen titles 32px/800, the lede 28px/500
with one 800 word. Body 16 to 19px, metadata 13 to 15px.

Layout: no boxes around screens; content sits on the ground. Order lists use hairline rules.
Buttons are pills; the tomato one is the single primary action per screen. Margin 16px.

## Signature

When the order moves to a new step, that step's symbol is stamped: it lands from 135% scale and
-8 degrees, overshoots to 94%, and settles (320 ms). Each step stamps once per visit. With reduced
motion the symbol simply appears.

## Checked

Rendered at 402x874 and 375x667 (light and dark) with the preview server; screenshots of the
amount, pay, verify, received, expired, order list and seller screens. Pay and verify were shot
with a made-up order injected into the local RPC response only.
