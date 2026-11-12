# Deploy the web page (Vercel, free Hobby plan)

**Live:** https://near-momo-p2p.vercel.app (deployed Nov 12 from `main`, root directory `web`).
Every push to `main` redeploys it.

The page in `web/` is a static site: `npm run build` writes `web/dist`, and Vercel serves it.
No server, no secrets: it reads public chain data over NEAR RPC, and every action is signed in the
visitor's own wallet. `web/vercel.json` holds the build settings (Vite, `npm ci`, `npm run build`,
output `dist`), per https://vercel.com/docs/project-configuration.

Checked before writing this: a clean `npm ci && npm test && npm run build` from a fresh copy of
`web/` (Node 22) passes and produces `dist/` with relative asset paths.

## Option A: Vercel dashboard (recommended)
1. Sign in at vercel.com with GitHub (Hobby plan is free).
2. **Add New → Project**, import `Ikwame91/near-momo-p2p`.
3. Set **Root Directory** to `web`. Vercel then reads `web/vercel.json`; framework shows **Vite**.
4. **Production branch**: `main` (merge `web/buyer-page` first), or deploy the branch as a preview.
5. Environment variables: none needed. The defaults point at `escrow-d2.ikwame.testnet`, seller
   `ikwame.testnet`, RPC `https://test.rpc.fastnear.com`. To change them, add `VITE_ESCROW_ID`,
   `VITE_DEFAULT_LP`, `VITE_RPC_URL` (they are public values, built into the page).
6. **Deploy**. You get `https://<project>.vercel.app`. Every push to the production branch redeploys.

## Option B: Vercel CLI
```bash
cd web
npx vercel@latest login
npx vercel@latest          # first run links the project (choose the defaults); makes a preview URL
npx vercel@latest --prod   # production URL
```
Run it from `web/`, so the project root is `web`.

## After it's live
- Open the URL in a fresh browser: the seller card and price should load within a few seconds.
- `?seller` shows the seller page; `?view=buyer-d2.ikwame.testnet` shows a paid order read-only.
- Connect Meteor (testnet) and check the login window opens. MyNearWallet is not offered: it
  stopped new app connections on Nov 1 2026 (D-024).
- Keep it up until judging ends (PLAN M4.5: until Nov 25).
