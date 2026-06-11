//! Cycle-measurement binary: artifacts are embedded at compile time so the
//! program runs under a bare ckb-vm runner with no transaction context.
//!
//! Run `cargo run --profile build-fast -p jolt-proof-export -- --guest
//! fibonacci --out target/jolt-artifacts` at the repo root first; the
//! `include_bytes!` paths below point into that output directory.

#![no_std]
#![no_main]

mod sync_shims;
mod verify;

use ckb_std::{debug, default_alloc, entry};

entry!(main);
default_alloc!({ 4 * 1024 }, { 2304 * 1024 }, 64);

const PREPROCESSING: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../target/jolt-artifacts/preprocessing.bin"
));
const PUBLIC_IO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../target/jolt-artifacts/public_io.bin"
));
const PROOF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../target/jolt-artifacts/proof.bin"
));

fn main() -> i8 {
    let code = verify::verify_artifacts(PREPROCESSING, PUBLIC_IO, PROOF);
    debug!("jolt-verify-bench exit code: {}", code);
    code
}
