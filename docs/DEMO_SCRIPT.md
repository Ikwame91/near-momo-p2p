# Demo video script (2–3 minutes)

Two versions: **Plan A** if OutLayer testnet is working when you record, **Plan B** if it is
down. Plan B uses only things that already happened on-chain and says so on screen; it never
pretends a recording is live. Both start with the same 20-second story. The brief for an editor
or an AI video tool is in [VIDEO_BRIEF.md](VIDEO_BRIEF.md).

Before recording (both plans):
- Browser zoom 100 %, a phone-sized window (about 420 px wide) or a real phone.
- Tabs ready: https://near-momo-p2p.vercel.app, Meteor unlocked on **testnet** with
  `firmstreet.testnet` active (about 0.5 NEAR is enough for one order), nearblocks.
- Close other Meteor tabs. Paystack test number **0551234987** (MTN).
- Check OutLayer just before (ask Claude to run the probe). If "Open Paystack checkout" shows
  "Failed to fetch" in Meteor, stop and switch to Plan B.

## The opening (0:00–0:20, both plans)

On screen: simple illustrations or text cards (see VIDEO_BRIEF.md), no app yet.

> "Ama in Kumasi has cedis on MoMo and wants digital dollars. Kwesi, a freelancer, is paid in
> USDC and needs cedis for rent. Today they meet in a WhatsApp group, and one of them has to send
> first and hope the other is honest. OpenRamp removes that leap of faith: the dollars are locked
> in a NEAR contract and released only when Paystack itself confirms the MoMo payment."

## Plan A: live (OutLayer working)

| Time | Screen | Say (roughly) |
|---|---|---|
| 0:20–0:35 | Buy screen: the adinkra band, "It stays **sealed** until Paystack sees your MoMo" | "I'm Ama. This is the seller's offer: 15 cedis per USDC, and a 50 USDC bond if they cheat." |
| 0:35–0:50 | Tap 1, see "You pay GH₵ 15.00"; point at the fees line; **Open order**, approve in Meteor | "I pick 1 USDC. The contract locks the seller's USDC for me. Fees are shown before I confirm." |
| 0:50–1:10 | Pay screen: the second symbol stamps, countdown, Copy buttons; **Open Paystack checkout**, approve | "A program in a secure enclave creates the Paystack checkout with the seller's own key. The key never leaves the enclave." |
| 1:10–1:30 | Paystack page: MTN test MoMo until "successful" | "I pay in cedis by mobile money. Test mode, so no real money." |
| 1:30–1:55 | **I've paid, check payment**; "Verifying…" (third symbol); "USDC received", amount counts up | "The enclave asks Paystack: was this exact order paid? Only a yes for this order releases the USDC." |
| 1:55–2:10 | **Withdraw to my wallet**; **View transaction** opens nearblocks | "It's in my wallet. Every step is a NEAR transaction anyone can check." |
| 2:10–2:30 | `?seller` page: 50 USDC guarantee, amount available, key fingerprint | "If the seller cancels their Paystack key to block the check, it can't be verified, and after a short window their bond pays me. No dispute desk: the rule is in the contract." |
| 2:30–2:50 | README "Who is this for" and "Honest limits" | "Testnet and Paystack test mode today. Real money needs licensed sellers; OpenRamp is the trust layer between them and people like Ama." |

## Plan B: OutLayer down (no live checks)

Say once after the opening: **"OutLayer's testnet has been unstable today, so I'll show a
purchase I made earlier, on November 13, and the page in read-only mode."**

| Time | Screen | Say (roughly) |
|---|---|---|
| 0:20–0:35 | Buy screen: type 1, then 5; GH₵ updates; the bond line and fees | "You pick an amount, see the cedi price, the seller's bond and the fees." |
| 0:35–1:05 | `?view=firmstreet.testnet`, tap the **Paid 2 USDC** order (order 8): all four symbols stamped, "USDC received" | "This is a real order: 2 USDC for 30 cedis, paid through Paystack's checkout page, confirmed by the enclave." |
| 1:05–1:40 | **View on the explorer** (the escrow account on nearblocks), the Nov 13 ~19:05 UTC transactions from `firmstreet.testnet`: open → checkout (the `order_checkout` event) → claim (`order_released`) → withdraw | "The contract released the USDC only after the enclave's Paystack check said this order was paid." |
| 1:40–2:00 | An **Expired** order in the list | "If nobody pays in time, the order expires and the seller's USDC unlocks. Nobody loses anything." |
| 2:00–2:25 | The README slash row on nearblocks: `order_slashed` | "If the seller's key is rejected, the bond pays the buyer automatically." |
| 2:25–2:50 | `?seller` page, then "Honest limits" | Same close as Plan A. |

## Don't
- Don't press "Check payment" or "Open Paystack checkout" while OutLayer is down: a check that
  can't be verified counts toward slashing the seller (by design).
- Don't show a wallet's recovery phrase, a private key, a password, or the Paystack dashboard.
- Don't say "no owner" or "nobody can change it": the escrow account still has its upgrade key on
  this testnet deploy (README, Upgrade keys). Say "no admin functions" or "no dispute desk".
