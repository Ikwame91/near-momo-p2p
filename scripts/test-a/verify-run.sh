#!/usr/bin/env bash
# Test A part 3 (M0.5): ask test-a.ikwame.testnet to run paystack-verify.wasm on OutLayer and
# print what the contract's callback stored.
# Usage: scripts/test-a/verify-run.sh <secrets-profile> <reference> [expected_pesewas] [pay_deadline_unix]
# Needs NEAR_PRIVATE_KEY (signs as ikwame.testnet). The key is never printed.
set -euo pipefail
PROFILE="$1"; REF="$2"; AMOUNT="${3:-1000}"; DEADLINE="${4:-$(( $(date +%s) + 600 ))}"
CONTRACT=test-a.ikwame.testnet
SIGNER=ikwame.testnet
# Pinned build, served from this repo (DECISIONS D-008).
WASM_URL=https://raw.githubusercontent.com/Ikwame91/near-momo-p2p/14ff01bc6b2786300414a2d449074b78c5ea7cf6/wasm/paystack-verify.wasm
WASM_HASH=021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61
ARGS=$(python3 -c 'import json,sys
u,h,ref,amt,dl,prof,acct=sys.argv[1:]
print(json.dumps({"wasm_url":u,"wasm_hash":h,
  "input":json.dumps({"reference":ref,"expected_amount_pesewas":int(amt),"currency":"GHS","pay_deadline":int(dl)}),
  "secrets_ref":{"profile":prof,"account_id":acct}}))' "$WASM_URL" "$WASM_HASH" "$REF" "$AMOUNT" "$DEADLINE" "$PROFILE" "$SIGNER")
near contract call-function as-transaction "$CONTRACT" request json-args "$ARGS" \
  prepaid-gas '300 Tgas' attached-deposit '0.01 NEAR' sign-as "$SIGNER" network-config testnet \
  sign-with-plaintext-private-key "$NEAR_PRIVATE_KEY" send 2>&1 \
  | sed -E 's/ed25519:[1-9A-HJ-NP-Za-km-z]{60,}/<redacted>/g' \
  | grep -E 'Transaction ID|Execution failed|^"' || true
