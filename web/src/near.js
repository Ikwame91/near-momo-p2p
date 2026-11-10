// NEAR access: read-only views over JSON-RPC, and signed calls through the user's wallet.
// RPC `query` / `call_function`: https://docs.near.org/api/rpc/contracts#call-a-contract-function
// Wallet Selector: https://github.com/near/wallet-selector (packages/core README, v10)
import { setupWalletSelector } from "@near-wallet-selector/core";
import { setupModal } from "@near-wallet-selector/modal-ui-js";
import "@near-wallet-selector/modal-ui-js/styles.css";
import { setupIntearWallet } from "@near-wallet-selector/intear-wallet";
import { setupMeteorWallet } from "@near-wallet-selector/meteor-wallet";
import { toTransaction } from "./calls.js";
import { ESCROW_ID, RPC_URL } from "./config.js";

/** Call a view method; returns the decoded JSON result. Throws on any RPC or contract error. */
export async function view(contractId, method, args = {}) {
  const res = await fetch(RPC_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "query",
      params: {
        request_type: "call_function",
        finality: "optimistic",
        account_id: contractId,
        method_name: method,
        args_base64: btoa(JSON.stringify(args)),
      },
    }),
  });
  if (!res.ok) throw new Error(`NEAR RPC returned HTTP ${res.status}`);
  const body = await res.json();
  if (body.error) throw new Error(body.error.data || body.error.message || "NEAR RPC error");
  if (body.result?.error) throw new Error(body.result.error);
  return JSON.parse(new TextDecoder().decode(Uint8Array.from(body.result.result)));
}

let selector;
let modal;

/** Set up the wallet selector; `onChange(accountId | null)` fires now and on every sign in/out. */
export async function initWallet(onChange) {
  selector = await setupWalletSelector({
    network: { networkId: "testnet", nodeUrl: RPC_URL, helperUrl: "https://helper.testnet.near.org", explorerUrl: "https://testnet.nearblocks.io", indexerUrl: "" },
    // MyNearWallet is not offered: it stopped app connections in Nov 2026 ("limited mode", seen in a test).
    modules: [setupMeteorWallet(), setupIntearWallet()],
  });
  // No function-call access key is requested: every step attaches NEAR, so the wallet confirms each one.
  modal = setupModal(selector, { contractId: ESCROW_ID, description: "Testnet only. Pick the wallet that holds your NEAR testnet account." });
  const active = (state) => state.accounts.find((a) => a.active)?.accountId || null;
  selector.store.observable.subscribe((state) => onChange(active(state)));
  onChange(active(selector.store.getState()));
}

export function showWalletModal() {
  modal.show();
}

export async function signOut() {
  const wallet = await selector.wallet();
  await wallet.signOut();
}

/**
 * Sign and send one or more function calls, each its own transaction.
 * `calls`: plain call objects from ./calls.js.
 * Returns the transaction hashes the wallet reports (may be empty for redirect wallets).
 */
export async function send(calls) {
  const wallet = await selector.wallet();
  const transactions = calls.map(toTransaction);
  const out =
    transactions.length === 1
      ? [await wallet.signAndSendTransaction(transactions[0])]
      : await wallet.signAndSendTransactions({ transactions });
  return (out || []).filter(Boolean).map((o) => o.transaction?.hash).filter(Boolean);
}
