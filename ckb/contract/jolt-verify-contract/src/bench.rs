//! Cycle-measurement binary: artifacts are embedded at compile time so the
//! program runs under a bare ckb-vm runner with no transaction context.
//!
//! Run `cargo run --profile build-fast -p jolt-proof-export -- --guest
//! fibonacci --out target/jolt-artifacts` at the repo root first; the
//! `include_bytes!` paths below point into that output directory.
//!
//! Emits one debug-syscall line per verification phase:
//! `phase <name> <cycles-since-previous-probe>`. Cycle counts come from the
//! `current_cycles` syscall (500 cycles each — negligible against G-scale
//! phases). The runner must implement syscalls 2042 and 2177.

#![no_std]
#![no_main]

mod sync_shims;
mod verify;

use ckb_std::syscalls::{current_cycles, debug};
use ckb_std::{default_alloc, entry};

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
    let mut last = current_cycles();
    let mut probe = |phase: &'static str| {
        let now = current_cycles();
        debug(alloc::format!("phase {} {}", phase, now - last));
        last = current_cycles();
    };

    let code = verify::verify_artifacts_with_probe(PREPROCESSING, PUBLIC_IO, PROOF, &mut probe);
    debug(alloc::format!("verify exit code: {}", code));
    code
}
