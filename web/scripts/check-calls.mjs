// Dev check: sign the page's exact contract calls (src/calls.js) with a TEST account key and send
// them to testnet, to prove method names, args, deposits and gas are what the escrow accepts.
// It cannot check the wallet UI itself. Never use a key that holds anything of value.
//
// Usage: node scripts/check-calls.mjs <account_id> <step> [order_id]
//   steps: open | checkout | claim | expire | withdraw
// Key: ~/.near-credentials/testnet/<account_id>.json (near-cli-rs legacy keychain). Never printed.
// @near-js/accounts Account: https://github.com/near/near-api-js/tree/master/packages/accounts
import fs from "node:fs";
import os from "node:os";
import { Account } from "@near-js/accounts";
import { JsonRpcProvider } from "@near-js/providers";
import { KeyPairSigner } from "@near-js/signers";
import { calls, storageDeposit, toTransaction } from "../src/calls.js";
import { ghsPesewasFor } from "../src/format.js";

const ESCROW = process.env.VITE_ESCROW_ID || "escrow-d2.ikwame.testnet";
const LP = process.env.VITE_DEFAULT_LP || "ikwame.testnet";
const RPC = process.env.VITE_RPC_URL || "https://test.rpc.fastnear.com";
const [accountId, step, orderArg] = process.argv.slice(2);
if (!accountId || !step) throw new Error("usage: check-calls.mjs <account_id> <step> [order_id]");

const cred = JSON.parse(fs.readFileSync(`${os.homedir()}/.near-credentials/testnet/${accountId}.json`, "utf8"));
const provider = new JsonRpcProvider({ url: RPC });
const account = new Account(accountId, provider, KeyPairSigner.fromSecretKey(cred.private_key));
const view = (contractId, method, args = {}) => provider.callFunction(contractId, method, args);
const escrow = calls(ESCROW);
const orderId = orderArg === undefined ? undefined : Number(orderArg);

const config = await view(ESCROW, "get_config");
const fee = config.params.outlayer_call.fee;
let todo;
if (step === "open") {
  const lp = await view(ESCROW, "get_lp", { account_id: LP });
  const units = BigInt(lp.quote.min_usdc);
  todo = [escrow.openOrder(LP, units, ghsPesewasFor(units, BigInt(lp.quote.rate_pesewas_per_usdc)))];
} else if (step === "checkout") todo = [escrow.requestCheckout(orderId, fee)];
else if (step === "claim") todo = [escrow.claim(orderId, fee)];
else if (step === "expire") todo = [escrow.expireOrder(orderId)];
else if (step === "withdraw") {
  todo = [];
  if (!(await view(config.token, "storage_balance_of", { account_id: accountId }))) {
    todo.push(storageDeposit(config.token, accountId, (await view(config.token, "storage_balance_bounds")).min));
  }
  todo.push(escrow.withdrawClaimable(accountId));
} else throw new Error(`unknown step ${step}`);

for (const c of todo) {
  const out = await account.signAndSendTransaction({ ...toTransaction(c), waitUntil: "FINAL" });
  const logs = out.receipts_outcome.flatMap((r) => r.outcome.logs).filter((l) => l.includes('"standard":"openramp"'));
  console.log(`${c.method}: tx ${out.transaction.hash}`);
  for (const l of logs) console.log("  " + l.replace("EVENT_JSON:", ""));
}
