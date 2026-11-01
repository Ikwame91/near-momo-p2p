#!/bin/sh
set -e
cargo build --target wasm32-wasip2 --release
mkdir -p out
cp target/wasm32-wasip2/release/paystack-verify.wasm out/
sha256sum out/paystack-verify.wasm
