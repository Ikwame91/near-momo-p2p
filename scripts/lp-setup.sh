#!/usr/bin/env bash
# Set up an LP on a testnet escrow (PLAN M2.8): mint test mUSDC, register_lp pointing at the LP's
# own OutLayer secret profile, deposit bond + liquidity, then ask the TEE to fingerprint the key.
#
# Usage: scripts/lp-setup.sh <tag> <lp_account> <secret_profile>
#   <tag> picks musdc-<tag>.<parent> / escrow-<tag>.<parent> from scripts/deploy-testnet.sh.
# SKIP_FINGERPRINT=1 stops before request_fingerprint (run it once the secret is stored).
# Needs: NEAR_ACCOUNT_ID (parent), NEAR_PRIVATE_KEY (its full-access key; the LP must be the parent
# or a sub-account created with the same key). The secret itself is stored separately by the LP
# (`outlayer secrets set ... --profile <secret_profile> --wasm-hash <verifier hash>
#  --access allow-all --via escrow-<tag>.<parent>`); this script never sees it. Key never printed.
set -euo pipefail
TAG="${1:?usage: lp-setup.sh <tag> <lp_account> <secret_profile>}"
LP="${2:?lp account}"
PROFILE="${3:?secret profile}"
PARENT="${NEAR_ACCOUNT_ID:?set NEAR_ACCOUNT_ID}"
: "${NEAR_PRIVATE_KEY:?set NEAR_PRIVATE_KEY}"
TOKEN="musdc-$TAG.$PARENT"
ESCROW="escrow-$TAG.$PARENT"
BOND="${BOND:-50000000}"            # 50 mUSDC (6 decimals)
LIQUIDITY="${LIQUIDITY:-100000000}" # 100 mUSDC
REDACT='s/ed25519:[1-9A-HJ-NP-Za-km-z]{60,}/<redacted>/g'

near_send() { "$@" network-config testnet sign-with-plaintext-private-key "$NEAR_PRIVATE_KEY" send 2>&1 \
  | sed -E "$REDACT" | grep -E 'Transaction ID|successfully|rror|panicked' || true; }
call() { local who=$1 contract=$2 method=$3 args=$4 deposit=$5 gas=${6:-100 Tgas}
  near_send near contract call-function as-transaction "$contract" "$method" json-args "$args" \
    prepaid-gas "$gas" attached-deposit "$deposit" sign-as "$who"; }

echo "== mint $((BOND + LIQUIDITY)) units of mUSDC to $LP (test token only)"
call "$LP" "$TOKEN" mint "{\"account_id\":\"$LP\",\"amount\":\"$((BOND + LIQUIDITY))\"}" '0.01 NEAR' '30 Tgas'

echo "== register_lp (paystack, profile $PROFILE)"
QUOTE='{"rate_pesewas_per_usdc":"1500","min_usdc":"1000000","max_usdc":"50000000","new_buyer_exposure_max":"100000000"}'
call "$LP" "$ESCROW" register_lp "{\"provider\":\"paystack\",\"secret_ref\":{\"profile\":\"$PROFILE\",\"account_id\":\"$LP\"},\"quote\":$QUOTE}" '0.1 NEAR' '30 Tgas'

echo "== bond + liquidity"
call "$LP" "$TOKEN" ft_transfer_call "{\"receiver_id\":\"$ESCROW\",\"amount\":\"$BOND\",\"msg\":\"bond\"}" '1 yoctoNEAR'
call "$LP" "$TOKEN" ft_transfer_call "{\"receiver_id\":\"$ESCROW\",\"amount\":\"$LIQUIDITY\",\"msg\":\"liquidity\"}" '1 yoctoNEAR'

[ "${SKIP_FINGERPRINT:-}" = 1 ] && { echo "== fingerprint skipped (SKIP_FINGERPRINT=1)"; exit 0; }
echo "== request_fingerprint (OutLayer TEE reads the LP's secret; fee 0.01 NEAR, excess refunded)"
call "$LP" "$ESCROW" request_fingerprint '{}' '0.05 NEAR' '300 Tgas'
