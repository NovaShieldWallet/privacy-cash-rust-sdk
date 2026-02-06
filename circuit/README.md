# Circuit files

This SDK expects the Privacy Cash circuit artifacts to be available locally for **pure-Rust** Groth16 proof generation.

This repo includes the required circuit artifacts in this directory:

- `transaction2.wasm`
- `transaction2.zkey`

The SDK default circuit base path is `./circuit/transaction2`, so it will look for:

- `./circuit/transaction2.wasm`
- `./circuit/transaction2.zkey`

## Provenance & License

These circuit artifacts were copied from the upstream Privacy Cash repository:

- Source repo: https://github.com/Privacy-Cash/privacy-cash
- Source path: `artifacts/circuits/transaction2.{wasm,zkey}`
- Source commit: `cbc83788cd6a3cf12fe03f6e97cc618cccd174e1`

**Important:** The upstream repo is licensed under **BSL 1.1** (transitioning to **GPL-2.0-or-later** on 12/27/2027). These circuit artifacts are **not** covered by this SDK repo’s MIT license—ensure your intended use complies with the upstream license terms.
