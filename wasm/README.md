# wasm/

Compiled OutLayer programs, served to OutLayer workers from their raw GitHub URL
(FastFS testnet was down, see DECISIONS.md D-008). Each file is checked by sha256 on the worker.
No secrets here: secrets reach programs only as OutLayer secrets at run time.

| File | Source | sha256 |
|---|---|---|
| hello.wasm | scripts/test-a/hello (`./build.sh`) | 0ca1af08790c4d29256766cb7193840c908339e3353221e397418814cdfaa36b |
| paystack-verify.wasm | scripts/test-a/paystack-verify (`./build.sh`) | 021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61 |
| openramp-verifier.wasm | verifier/ (`./build.sh`) | 6a9ff46b643c65ef7339c5d6b7f573b55c09f7764c614c101c958d76a440926f |
