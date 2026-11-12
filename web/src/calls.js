// The contract calls this page makes, as plain data, in one place. Used by the page (through the
// wallet) and by scripts/check-calls.mjs (signed with a test key) so both send exactly the same thing.
import { actionCreators } from "@near-js/transactions";

const TGAS = 1_000_000_000_000n;
export const OPEN_ORDER_DEPOSIT = "100000000000000000000000"; // 0.1 NEAR for order storage; the contract refunds the excess

const call = (receiverId, method, args, depositYocto, gasTgas) => ({ receiverId, method, args, depositYocto: String(depositYocto), gasTgas });

/** `fee` is the escrow's OutLayer fee in yoctoNEAR (get_config → params.outlayer_call.fee). */
export const calls = (escrowId) => ({
  openOrder: (lp, usdcUnits, maxGhsPesewas) =>
    call(escrowId, "open_buy_order", { lp, usdc_amount: String(usdcUnits), max_ghs_amount_pesewas: String(maxGhsPesewas) }, OPEN_ORDER_DEPOSIT, 30),
  requestCheckout: (orderId, fee) => call(escrowId, "request_checkout", { order_id: orderId }, fee, 300),
  claim: (orderId, fee) => call(escrowId, "claim", { order_id: orderId }, fee, 300),
  finalizeRelease: (orderId) => call(escrowId, "finalize_release", { order_id: orderId }, 0, 30),
  expireOrder: (orderId) => call(escrowId, "expire_order", { order_id: orderId }, 0, 30),
  withdrawClaimable: (account) => call(escrowId, "withdraw_claimable", { account }, 0, 100),
});

/** NEP-145 registration on the token, needed once before an account can receive it. */
export const storageDeposit = (tokenId, account, minYocto) => call(tokenId, "storage_deposit", { account_id: account }, minYocto, 30);

export function toTransaction(c) {
  return {
    receiverId: c.receiverId,
    actions: [actionCreators.functionCall(c.method, c.args, BigInt(c.gasTgas) * TGAS, BigInt(c.depositYocto))],
  };
}
