#!/usr/bin/env bash
# Buyer-side testnet steps (PLAN M2.3 / M2.4). Each step is one command so the demo can be paced.
#
# Usage: scripts/buy.sh <tag> <step> [args]
#   buyer                    create buyer-<tag>.<parent> (1 NEAR) and register it on the token
#   open <lp> <usdc_units>   open_buy_order (storage deposit attached, excess refunded)
#   checkout <order_id>      TEE creates the Paystack checkout with the LP's key
#   claim <order_id>         TEE checks the payment; verdict arrives in the OutLayer callback
#   expire <order_id>        expire an unpaid order after pay_deadline + t_grace (frees the LP's USDC)
#   withdraw                 pay the buyer's claimable mUSDC out
#   order <order_id>         view the order;  claimable  view the buyer's claimable
# Needs: NEAR_ACCOUNT_ID (parent), NEAR_PRIVATE_KEY (the buyer is a sub-account with the same key).
set -euo pipefail
TAG="${1:?usage: buy.sh <tag> <step> [args]}"
STEP="${2:?step}"
PARENT="${NEAR_ACCOUNT_ID:?set NEAR_ACCOUNT_ID}"
: "${NEAR_PRIVATE_KEY:?set NEAR_PRIVATE_KEY}"
TOKEN="musdc-$TAG.$PARENT"
ESCROW="escrow-$TAG.$PARENT"
BUYER="buyer-$TAG.$PARENT"
REDACT='s/ed25519:[1-9A-HJ-NP-Za-km-z]{60,}/<redacted>/g'

near_send() { "$@" network-config testnet sign-with-plaintext-private-key "$NEAR_PRIVATE_KEY" send 2>&1 \
  | sed -E "$REDACT" | grep -E 'Transaction ID|successfully|rror|panicked|EVENT_JSON:\{"standard":"openramp"' || true; }
call() { near_send near contract call-function as-transaction "$ESCROW" "$1" json-args "$2" \
  prepaid-gas "${4:-100 Tgas}" attached-deposit "$3" sign-as "$BUYER"; }
view() { near contract call-function as-read-only "$ESCROW" "$1" json-args "$2" network-config testnet now 2>&1 \
  | sed -n '/return value/,/^Here is your console/p' | sed '1d;$d'; }

case "$STEP" in
  buyer)
    PUB=$(outlayer whoami | awk '/^Key:/{print $2}')
    near_send near account create-account fund-myself "$BUYER" '1 NEAR' use-manually-provided-public-key "$PUB" sign-as "$PARENT"
    near_send near contract call-function as-transaction "$TOKEN" storage_deposit json-args "{\"account_id\":\"$BUYER\"}" \
      prepaid-gas '30 Tgas' attached-deposit '0.01 NEAR' sign-as "$PARENT" ;;
  open) call open_buy_order "{\"lp\":\"${3:?lp}\",\"usdc_amount\":\"${4:?usdc units}\"}" '0.1 NEAR' '30 Tgas' ;;
  checkout) call request_checkout "{\"order_id\":${3:?order id}}" '0.05 NEAR' '300 Tgas' ;;
  claim) call claim "{\"order_id\":${3:?order id}}" '0.05 NEAR' '300 Tgas' ;;
  expire) call expire_order "{\"order_id\":${3:?order id}}" '0 NEAR' '30 Tgas' ;;
  withdraw) call withdraw_claimable "{\"account\":\"$BUYER\"}" '0 NEAR' '100 Tgas' ;;
  order) view get_order "{\"order_id\":${3:?order id}}" ;;
  claimable) view get_claimable "{\"account\":\"$BUYER\"}" ;;
  *) echo "unknown step $STEP" >&2; exit 2 ;;
esac
