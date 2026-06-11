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
- **jolt-verify-nostd-check** — `#![no_std]` library that monomorphizes the
  full artifact-decode + verify flow. The no_std gate:
  `cargo check -p jolt-verify-nostd-check --target riscv64imac-unknown-none-elf`.
- **vendor/dory-pcs**, **vendor/dory-derive** — vendored fork of the external
  Dory PCS (crates.io 0.3.0) wired in via `[patch.crates-io]`. Changes: `std`
  feature (default on) with a no_std verify path (`ark_std::io` instead of
  `std::io`, alloc imports, `len.ilog2()`), Allocative/getrandom decoupled,
  `random()` gated to std (prover/setup only), unused `bincode` dropped.
- **contract/** — separate cargo workspace (own `[patch.crates-io]` table,
  kept in lockstep with the root) building the actual CKB script:
  - `jolt-verify-contract` — ckb-std entry, loads the three artifacts from
    transaction witnesses (provisional layout: group-input witnesses 0/1/2).
  - `jolt-verify-bench` — same flow with artifacts embedded via
    `include_bytes!` so it runs under a bare ckb-vm runner for cycle
    measurement (`--features bench-embedded`).

## Usage

```bash
# Host: prove and export (build-fast avoids a rustc fat-LTO stack overflow on Windows)
cargo run --profile build-fast -p jolt-proof-export -- --guest fibonacci --out target/jolt-artifacts

# Verify with the standalone stack only
cargo run --profile build-fast -p jolt-verify-smoke -- target/jolt-artifacts

# Dependency isolation guard
bash ckb/check-isolation.sh

# no_std gate: the whole verify stack on bare-metal riscv64
cargo check -p jolt-verify-nostd-check --target riscv64imac-unknown-none-elf

# Build the CKB script + bench binary (separate workspace; .cargo/config.toml
# pins the riscv64imac target and -C target-feature=-a,+forced-atomics)
cd ckb/contract && cargo build --release -p jolt-verify-contract --features bench-embedded

# Measure cycles in ckb-vm (use the local ckb-vm repo's runner; asm64 mode is
# faster wall-clock, interpreter64 needs no C toolchain — cycle counts match)
cd ../../../ckb-vm
cargo run --release --example ckb_vm_runner -- --mode interpreter64 \
  ../jolt/ckb/contract/target/riscv64imac-unknown-none-elf/release/jolt-verify-bench
```

## ckb-vm status (fibonacci proof, guest input 100)

The bench binary verifies the exported fibonacci proof inside ckb-vm:
`exit=Ok(0)`, **3.75 G cycles** (release, opt-level=s). Reference points:
CKB's `MAX_BLOCK_CYCLES` is 3.5 G and a typical two-in-two-out transfer is
3.5 M cycles — verification currently costs slightly more than an entire
block, so heavy cycle optimization (Straus-Shamir multi-exp, GT exponentiation
windows, pairing tuning) is the next phase's core work.

Memory: the VM gives a script 4 MB total. The ELF is ~1.5 MB text
(opt-level=s; opt-level=3 is ~2.1 MB and forces the heap below the
verifier's working set) + 2.3 MB buddy heap + ~190 KB stack headroom.
Host-measured peak live heap for this verification is ~1.6 MiB
(`jolt-verify-smoke` prints heap stats). Larger guests/traces will push both
the artifact sizes and the working set up — re-measure with bigger fixtures
before trusting these margins.

Build notes:
- `-C target-feature=-a,+forced-atomics`: mainnet VMs run IMC+B+MOP without
  atomics. `-a` alone makes LLVM crash on lowering some atomic RMW ops
  (`Cannot select: AtomicLoadAdd`); `+forced-atomics` lowers them to libcalls
  instead — `__atomic_*` come from ckb-std's `dummy-atomic`, and the few
  `__sync_*` ones (from `Arc` in serial-rayon fallbacks) from
  `src/sync_shims.rs`. rustc warns the flag combination is "unsound" because
  it changes the atomics ABI — irrelevant here since every object in the
  final ELF is built with the same flags and the VM is single-threaded.
- For panic messages in release builds add `-C debug-assertions` to
  RUSTFLAGS (enables ckb-std's `debug!`; costs ~2x cycles).

## Feature wiring

The verifier-stack crates expose `parallel` features (default on) so the VM
build can opt out of rayon: `jolt-verify-smoke` depends on `jolt-verifier`,
`jolt-dory`, and `jolt-crypto` with `default-features = false`. The `dory-pcs`
dependency of `jolt-dory` is declared with only the `arkworks` + `zk` features;
disk caching of SRS generation is behind jolt-dory's `srs-cache` feature
(default on, host-only concern).

Every crate in the verify stack additionally carries a `std` feature (default
on, forwarded down the dependency tree) and is `#![cfg_attr(not(feature =
"std"), no_std)]` + `extern crate alloc`. Conventions used throughout:
`core::`/`alloc::` paths instead of `std::`, no `std::sync`/`std::io` in the
verify path, `light-poseidon` (std-only) stays behind the `poseidon` transcript
feature, and `jolt-program`'s ELF parsing (`image` feature) implies `std`.
`ckb/fix-alloc-imports.py` is the helper that mechanically inserted alloc
imports from rustc errors during the migration; kept for future rebases.

## Known issues at the pin (7f97cbad)

- `jolt-verifier` rejects valid proofs that use advice polynomials:
  `StageClaimOutputMismatch { stage: HammingWeightClaimReduction }`
  (upstream bug, advice path only; reproduced with
  `cargo nextest run -p jolt-verifier --features core-fixtures -E 'test(advice_consumer)'`).
- The `field-inline` feature of `jolt-verifier` does not compile (upstream,
  pre-existing).
- `cargo check --workspace` fails on Windows in `zeroos-vfs-core`
  (unix-only guest runtime dependency, unrelated to verification).
