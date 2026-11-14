# Video brief: OpenRamp demo (for an editor or an AI video tool)

Paste everything below the line into the video tool, together with Kwame's screen recordings
of the live page. The script and timings are also in [DEMO_SCRIPT.md](DEMO_SCRIPT.md).

---

You are making the demo video for **OpenRamp**, a NEAR Hackathon 2026 submission (track:
Real-World Money). Length **2:30 to 2:50**. The audience is hackathon judges who have never heard
of the project: they must understand in the first 20 seconds **who it is for and what problem it
solves**, then see that it really works.

## The one idea to land

Two strangers in Ghana want to swap mobile money (MoMo) for digital dollars (USDC). Today one of
them has to send first and trust the other. OpenRamp locks the dollars in a NEAR contract and
releases them **only when Paystack itself confirms the MoMo payment**, checked by a sealed program
nobody can tamper with. If the seller cheats by blocking that check, their bond pays the buyer.
**Nobody has to trust anybody.**

## Characters (use these names and roles exactly)

- **Ama**, the buyer: a nurse in Kumasi with cedis on MTN MoMo. She wants dollars to save (the
  cedi loses value) or to pay for something priced in dollars.
- **Kwesi**, the seller: a freelance designer paid in USDC by a US client. His rent is in cedis.
- In the demo, Kwesi is the account `ikwame.testnet` and Ama is `firmstreet.testnet`.

## Look and feel

Match the app so the illustrations and the screen recordings feel like one film:

- Colours: cobalt blue background `#2340b0`, a lighter panel blue `#2d4cc6`, white text, one
  tomato accent `#ff5b3a` used only for the key moment on each scene (the action, the release).
  Night scenes may use navy `#0e1a47`.
- Type: **Bricolage Grotesque** (bold for headlines), sentence case, no all-caps titles.
- Motif: two real **adinkra** symbols from the app, white line art: **Dwennimmen** (four ram's
  horns curling out from a centre: strength with humility) and **Nkyinkyim** (a twisting zigzag
  ribbon: the twisting path). In the app one symbol is "stamped" per step, like adinkra cloth
  stamping: it drops in slightly large and rotated, overshoots, and settles. Reuse that stamp as
  the transition between story beats.
- Illustration style: flat, clean shapes, one hand, the palette above. Simple phones, a padlocked
  box for the contract, a wax-seal or stamp for "confirmed". Warm and specific to Ghana (a market
  stall, a trotro, a laptop at night) without flag colours, MTN yellow or cartoon stereotypes.
- Avoid: purple gradients, glowing "crypto" graphics, coins raining, stock-photo people, emoji,
  robotic or British-accented AI narration. A calm Ghanaian or neutral West African English voice
  (or Kwame's own voice) is best. Add burned-in captions; many judges watch muted.

## Scenes

| # | Time | Visual | Narration (keep it close to this) |
|---|---|---|---|
| 1 | 0:00–0:08 | Illustration: Ama on her phone with a MoMo balance; Kwesi at a laptop, a "USDC received" notification from a US client. | "Ama has cedis on mobile money and wants digital dollars. Kwesi is paid in USDC and needs cedis for rent." |
| 2 | 0:08–0:16 | A WhatsApp-style group chat ("Selling USDC, DM me"). Money arrow from Ama to Kwesi with a question mark; then the reverse arrow with a fake "payment received" SMS. Small red "?" on each. | "Today they meet in a group chat, and one of them has to send first and hope the other is honest." |
| 3 | 0:16–0:22 | Title card on cobalt: "OpenRamp" + "Mobile money to USDC, without trusting the seller." First Dwennimmen stamp lands. | "OpenRamp removes that leap of faith." |
| 4 | 0:22–0:40 | Diagram, built step by step: Kwesi's USDC + a bond go into a padlocked box labelled "NEAR contract". Ama's phone pays "GH₵ 15" to "Paystack". A sealed box labelled "secure enclave" asks Paystack "Was this order paid?"; a stamp says "Paid"; the padlock opens and USDC flows to Ama. | "Kwesi locks his USDC in a NEAR contract, with a bond. Ama pays by MoMo on Paystack's normal checkout. A sealed program asks Paystack directly whether this exact order was paid. Only a yes releases the USDC." |
| 5 | 0:40–1:55 | **Screen recording** of the live page (Kwame's clip). Overlay short captions on each step: "1 · Choose amount", "2 · Pay by MoMo", "3 · Paystack check", "4 · USDC received". Zoom gently on the adinkra band each time a symbol is stamped, on "GH₵ 15.00", on the countdown, and on the amount counting up. | Follow DEMO_SCRIPT.md Plan A (or Plan B if the clip is a past order: then say once "OutLayer's testnet was unstable, so this is a purchase I made earlier"). |
| 6 | 1:55–2:10 | Screen recording: "View transaction" opens the NEAR explorer. | "Every step is a NEAR transaction anyone can check." |
| 7 | 2:10–2:30 | Diagram: Kwesi cancels his Paystack key (a key icon breaks); the enclave's question gets no answer; a short timer runs out; the bond moves to Ama. Then the live seller page (`?seller`): "50 USDC guarantee". | "If the seller blocks the check, it can't be verified, and after a short window their bond pays the buyer. There's no dispute desk to call: the rule is in the contract." |
| 8 | 2:30–2:45 | Text card: "Testnet and Paystack test mode. No real money." then "Real money: licensed sellers plug into OpenRamp as the trust layer." | "Today it runs on testnet with Paystack test mode. With real money, licensed sellers plug in, and OpenRamp is the trust layer between them and people like Ama." |
| 9 | 2:45–2:50 | End card: the four symbols stamped in a row, "OpenRamp", `near-momo-p2p.vercel.app`, `github.com/Ikwame91/near-momo-p2p`. | (music only) |

## Facts you must keep exactly right

- Network: **NEAR testnet**. Payments: **Paystack test mode** (MTN test number 0551234987). No real
  money moves anywhere in the video.
- The checker is an **OutLayer** program in a **TEE** (secure enclave). It calls Paystack with the
  seller's own key; the key never leaves the enclave.
- Demo numbers: rate **GH₵ 15.00 per USDC**; seller bond **50 USDC**; a real purchase on Nov 13:
  **2 USDC for GH₵ 30.00** (order 8, released); the live recording on Nov 14 is **order 13**, also
  2 USDC for GH₵ 30.00, released (its four transactions are in the README proof table).
- Built: buy side only (cedis to USDC). The page is plain JavaScript on Vercel; the contract is
  Rust on NEAR with 270+ tests.
- Do **not** say: "no owner", "nobody can change the contract", "fully decentralized", "instant",
  "works with real money", or any user numbers. The testnet escrow account still has an upgrade
  key (the README says so). Safe wording: "no admin functions", "no dispute desk", "the rule is
  in the contract".
- Do not show or narrate wallet recovery phrases, passwords, private keys, or the Paystack
  dashboard. Blur anything like that if it appears in a recording.

## Deliverables

1. The 2:30–2:50 video, 1080x1920 (vertical, for X) **and** 1920x1080 (landscape, for the form),
   with burned-in captions.
2. A 20-second cut of scenes 1–4 for the X post.
3. A list of every claim made in the narration, so Kwame can check each one against this brief.
