# wasm/

Compiled OutLayer programs, served to OutLayer workers from their raw GitHub URL
(FastFS testnet was down, see DECISIONS.md D-008). Each file is checked by sha256 on the worker.
No secrets here: secrets reach programs only as OutLayer secrets at run time.

| File | Source | sha256 |
|---|---|---|
| hello.wasm | scripts/test-a/hello (`./build.sh`) | 0ca1af08790c4d29256766cb7193840c908339e3353221e397418814cdfaa36b |
