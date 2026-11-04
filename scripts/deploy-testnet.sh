#!/usr/bin/env bash
# Deploy OpenRamp to NEAR testnet (PLAN M2.8): mock USDC + escrow, with the escrow's deploy and
# `new` in ONE batched transaction signed by the escrow account itself (T32).
#
# Usage: scripts/deploy-testnet.sh <tag>
#   creates musdc-<tag>.<parent> and escrow-<tag>.<parent>, where <parent> = $NEAR_ACCOUNT_ID.
# Needs: NEAR_ACCOUNT_ID, NEAR_PRIVATE_KEY (the parent's full-access key; sub-accounts get the same
# public key). Contracts must be built first: (cd contracts/escrow && cargo near build
# non-reproducible-wasm) and the same for contracts/mock-ft. The key is never printed.
#
# Testnet demo parameters (SPEC §7 demo column). Verifier: the pinned wasm in this repo (D-008).
set -euo pipefail
TAG="${1:?usage: deploy-testnet.sh <tag>}"
PARENT="${NEAR_ACCOUNT_ID:?set NEAR_ACCOUNT_ID}"
: "${NEAR_PRIVATE_KEY:?set NEAR_PRIVATE_KEY}"
TOKEN="musdc-$TAG.$PARENT"
ESCROW="escrow-$TAG.$PARENT"
OUTLAYER="outlayer.testnet"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERIFIER_URL="${VERIFIER_URL:?set VERIFIER_URL (raw GitHub URL of wasm/openramp-verifier.wasm pinned to a commit)}"
VERIFIER_HASH="${VERIFIER_HASH:?set VERIFIER_HASH (sha256 of that wasm)}"
REDACT='s/ed25519:[1-9A-HJ-NP-Za-km-z]{60,}/<redacted>/g'
PUB=$(outlayer whoami | awk '/^Key:/{print $2}')   # the parent's public key (outlayer is logged in as the parent)

near_send() { "$@" network-config testnet sign-with-plaintext-private-key "$NEAR_PRIVATE_KEY" send 2>&1 \
  | sed -E "$REDACT" | grep -E 'Transaction ID|successfully|rror|panicked' || true; }

echo "== accounts"
near_send near account create-account fund-myself "$TOKEN" '2 NEAR' use-manually-provided-public-key "$PUB" sign-as "$PARENT"
near_send near account create-account fund-myself "$ESCROW" '5 NEAR' use-manually-provided-public-key "$PUB" sign-as "$PARENT"

echo "== mock USDC (test only)"
near_send near contract deploy "$TOKEN" use-file "$ROOT/contracts/target/near/openramp_mock_ft/openramp_mock_ft.wasm" \
  with-init-call new json-args '{}' prepaid-gas '30 Tgas' attached-deposit '0 NEAR'

echo "== escrow: deploy + new in one transaction, signed by the escrow account"
MIN=60000000000
INIT=$(python3 -c '
import json,sys
token,outlayer,url,h,m=sys.argv[1:]; m=int(m)
print(json.dumps({"token":token,"outlayer":outlayer,
  "verifier":{"WasmUrl":{"url":url,"hash":h}},
  "params":{"t_pay_ns":10*m,"t_grace_ns":2*m,"t_retry_ns":3*m,"t_outage_max_ns":6*m,
    "min_failed_attempts":2,"new_buyer_cap":"50000000","new_buyer_orders":3,"release_delay_ns":0,
    "outlayer_call":{"fee":"10000000000000000000000","gas_tgas":200,"callback_gas_tgas":30,
      "max_instructions":1000000000,"max_memory_mb":128,"max_execution_seconds":20}}}))' \
  "$TOKEN" "$OUTLAYER" "$VERIFIER_URL" "$VERIFIER_HASH" "$MIN")
near_send near contract deploy "$ESCROW" use-file "$ROOT/contracts/target/near/openramp_escrow/openramp_escrow.wasm" \
  with-init-call new json-args "$INIT" prepaid-gas '100 Tgas' attached-deposit '0 NEAR'

echo "== token storage for the escrow (so it can receive and send mUSDC)"
near_send near contract call-function as-transaction "$TOKEN" storage_deposit json-args "{\"account_id\":\"$ESCROW\"}" \
  prepaid-gas '30 Tgas' attached-deposit '0.01 NEAR' sign-as "$PARENT"

echo "== check"
near contract call-function as-read-only "$ESCROW" get_config json-args '{}' network-config testnet now 2>&1 \
  | python3 -c 'import sys,json; t=sys.stdin.read(); t=t[t.index("return value"):]; c,_=json.JSONDecoder().raw_decode(t[t.index("{"):]); print("token", c["token"], "| outlayer", c["outlayer"], "| deploy_tag", c["deploy_tag"], "| verifier", c["verifier"]["WasmUrl"]["hash"][:12])'
echo "TOKEN=$TOKEN"
echo "ESCROW=$ESCROW"
