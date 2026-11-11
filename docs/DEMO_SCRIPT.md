# Demo video script (2–3 minutes)

Two versions: **Plan A** if OutLayer testnet is working when you record, **Plan B** if it is
still down. Plan B uses only things that already happened on-chain, plus the page's read-only
views, and says so on screen; it never pretends a recording is live.

Before recording (both plans):
- Browser zoom 100 %, a phone-sized window (about 420 px wide) or a real phone.
- Tabs ready: the page (`npm run dev`, or the Vercel URL), Meteor unlocked on **testnet** with
  `firmstreet.testnet` active (about 0.7 NEAR, enough for one order), nearblocks.
- Close other Meteor tabs; Paystack test number **0551234987** (MTN).

## Plan A: live (OutLayer working)

| Time | Screen | Say (roughly) |
|---|---|---|
| 0:00–0:15 | Page, amount screen | "Cashing out crypto in Ghana means trusting a stranger with your money. OpenRamp lets you buy USDC with mobile money without trusting the seller." |
| 0:15–0:30 | Seller card, "50 USDC staked as guarantee" | "The seller's USDC is locked in a NEAR contract with no owner, and the seller has staked a bond." |
| 0:30–0:45 | Choose 1 USDC (GHS 15.00), Continue, approve in Meteor | "I open an order for 1 USDC. The contract locks the seller's USDC for me." |
| 0:45–1:10 | Open Paystack checkout, approve; Paystack page opens | "A program in a secure enclave creates the Paystack checkout with the seller's own key. The key never leaves the enclave." |
| 1:10–1:30 | Pay with MTN test MoMo until "successful" | "I pay in cedis by mobile money. Test mode: no real money." |
| 1:30–1:55 | I've paid, check payment; "Verifying…"; "USDC received" + confetti | "The enclave asks Paystack directly whether this exact order was paid. Only a 'paid' answer for this order releases the USDC." |
| 1:55–2:10 | Withdraw to my wallet; transaction link | "The USDC is in my wallet. Every step is a NEAR transaction anyone can check." |
| 2:10–2:35 | `?seller` page, then the slash row in the README table | "And if the seller revokes their Paystack key to cheat, the check can't be verified, and after a retry window the seller's bond pays the buyer. No admin, no dispute desk: the rule is in the contract." |
| 2:35–2:50 | README "Honest limits" | "Testnet and Paystack test mode today. Real money needs licensed sellers; OpenRamp is the trust layer between them and users." |

## Plan B: OutLayer down (no live checks)

Say once at the start: **"OutLayer's testnet is down today, so I'll show the flow with the
transactions we recorded on November 10, and the page in read-only mode."**

| Time | Screen | Say (roughly) |
|---|---|---|
| 0:00–0:15 | Page, amount screen | Same opening as Plan A. |
| 0:15–0:35 | Amount screen: type 1, then 5; GHS updates; seller card + guarantee badge | "You pick an amount, see the cedi price, and see the seller's staked guarantee." |
| 0:35–0:55 | `?view=buyer-d2.ikwame.testnet`, then click the order in the list: card "USDC received" | "This is a real order from November 10: 2 USDC for GHS 30, paid through Paystack's real checkout page." |
| 0:55–1:40 | nearblocks, one tab per row of the README "PAID through the real Paystack checkout page" table: fingerprint → open → checkout (show the `order_checkout` event with the Paystack URL) → claim (show `order_released`) → withdraw | "The enclave fingerprinted the seller's key, created the checkout, and after I paid, confirmed it with Paystack. The contract released the USDC only on that answer." |
| 1:40–2:05 | `?view=webtest-d2.ikwame.testnet`, click an Expired order: "This order expired" card (the pay countdown only shows if an unpaid order is open; skip it otherwise) | "If you don't pay in time, the order expires and the seller's USDC unlocks. Nobody loses anything." |
| 2:05–2:30 | README slash row on nearblocks: `order_slashed` event | "If the seller's key is rejected, the bond pays the buyer automatically. That's the guarantee." |
| 2:30–2:50 | `?seller` page, then "Honest limits" | "Bond, liquidity, key fingerprint, all public. Testnet and test mode today; depends on OutLayer being up, which it isn't right now." |

To show the slashed card on the page itself (optional): run the page against the old escrow,
`VITE_ESCROW_ID=escrow-d1.ikwame.testnet npm run dev`, and open `?view=buyer-d1.ikwame.testnet`.
Say it's the November 4 deploy.

## Don't
- Don't press "Check payment" or "Open Paystack checkout" on any order while OutLayer is down: a
  check that can't be verified counts toward slashing the seller (by design).
- Don't show a wallet's recovery phrase, a private key, or the Paystack dashboard's API keys page.
