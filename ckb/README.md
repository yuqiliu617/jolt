# ckb — Jolt verifier port toward ckb-vm

Foundation for running the Jolt verifier inside ckb-vm. The VM-side stack is
upstream's standalone `jolt-verifier` crate plus its modular dependency crates;
`jolt-core` participates only on the host side (proving and proof conversion).

## Crates

- **jolt-artifacts** — versioned container format (8-byte magic + postcard
  payload) for the artifacts exchanged between host and VM.
- **jolt-proof-export** — host tool (std): proves a guest with jolt-core,
  converts proof + preprocessing into the verifier model, self-checks with both
  verifiers, writes `preprocessing.bin`, `public_io.bin`, `proof.bin`.
- **jolt-verify-smoke** — verify-only consumer of the artifacts. Its dependency
  graph must stay free of prover/host machinery (no jolt-core, tracer,
  jolt-sdk, jolt-inlines, rayon); `check-isolation.sh` enforces this. This
  crate is the precursor of the ckb-vm contract.

## Usage

```bash
# Host: prove and export (build-fast avoids a rustc fat-LTO stack overflow on Windows)
cargo run --profile build-fast -p jolt-proof-export -- --guest fibonacci --out target/jolt-artifacts

# Verify with the standalone stack only
cargo run --profile build-fast -p jolt-verify-smoke -- target/jolt-artifacts

# Dependency isolation guard
bash ckb/check-isolation.sh
```

## Feature wiring

The verifier-stack crates expose `parallel` features (default on) so the VM
build can opt out of rayon: `jolt-verify-smoke` depends on `jolt-verifier`,
`jolt-dory`, and `jolt-crypto` with `default-features = false`. The `dory-pcs`
dependency of `jolt-dory` is declared with only the `arkworks` + `zk` features;
disk caching of SRS generation is behind jolt-dory's `srs-cache` feature
(default on, host-only concern).

## Known issues at the pin (7f97cbad)

- `jolt-verifier` rejects valid proofs that use advice polynomials:
  `StageClaimOutputMismatch { stage: HammingWeightClaimReduction }`
  (upstream bug, advice path only; reproduced with
  `cargo nextest run -p jolt-verifier --features core-fixtures -E 'test(advice_consumer)'`).
- The `field-inline` feature of `jolt-verifier` does not compile (upstream,
  pre-existing).
- `cargo check --workspace` fails on Windows in `zeroos-vfs-core`
  (unix-only guest runtime dependency, unrelated to verification).
