#!/bin/sh
# Build the OutLayer program (wasm32-wasip2) and print its sha256 (the escrow's verifier hash).
set -e
cargo build --target wasm32-wasip2 --release
mkdir -p out
cp target/wasm32-wasip2/release/openramp-verifier.wasm out/
sha256sum out/openramp-verifier.wasm
