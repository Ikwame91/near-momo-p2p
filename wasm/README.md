# wasm/

Compiled OutLayer programs, served to OutLayer workers from their raw GitHub URL
(FastFS testnet was down, see DECISIONS.md D-008). Each file is checked by sha256 on the worker.
No secrets here: secrets reach programs only as OutLayer secrets at run time.

| File | Source | sha256 |
|---|---|---|
| hello.wasm | scripts/test-a/hello (`./build.sh`) | 0ca1af08790c4d29256766cb7193840c908339e3353221e397418814cdfaa36b |
| paystack-verify.wasm | scripts/test-a/paystack-verify (`./build.sh`) | 021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61 |
| openramp-verifier.wasm | verifier/ (`./build.sh`) | ecc0515c0ddff734c7e4732b986b1fed3b95454ee8801f99b1622457ec62bf7b |

The escrow is pinned to one exact build. `escrow-d2` (current) uses `ecc0515c…`, the file above,
served from commit `14fd1f8`. The earlier `escrow-d1` used `6a9ff46b…`, served from commit `ccd6332`;
that build sent a checkout email Paystack rejects.
