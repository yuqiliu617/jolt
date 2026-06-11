//! CKB script entry: verifies a Jolt proof supplied via transaction witnesses.
//!
//! Provisional witness layout (foundation phase): the first three witnesses of
//! the input group hold the postcard artifacts produced by `jolt-proof-export`
//! — 0: preprocessing, 1: public io, 2: proof. A production deployment should
//! move the (per-program, reusable) preprocessing into a dep cell and pin its
//! digest in the script args; tracked for a later phase.

#![no_std]
#![no_main]
#![expect(
    clippy::unwrap_used,
    reason = "CKB scripts surface failures via panic -> exit code"
)]

mod sync_shims;
mod verify;

use ckb_std::ckb_constants::Source;
use ckb_std::high_level::load_witness;
use ckb_std::{default_alloc, entry};

entry!(main);
// The VM gives scripts 4 MB total (code + data + bss + stack). With ~1.5 MB
// of text (opt-level=s) this heap leaves ~190 KB of stack headroom. Host-side
// measurement of this verification path peaks at ~1.6 MiB of live
// allocations (`jolt-verify-smoke` prints heap stats); buddy-allocator
// rounding needs the extra margin.
default_alloc!({ 4 * 1024 }, { 2304 * 1024 }, 64);

const EXIT_MISSING_WITNESS: i8 = 20;

fn main() -> i8 {
    let Ok(preprocessing) = load_witness(0, Source::GroupInput) else {
        return EXIT_MISSING_WITNESS;
    };
    let Ok(public_io) = load_witness(1, Source::GroupInput) else {
        return EXIT_MISSING_WITNESS;
    };
    let Ok(proof) = load_witness(2, Source::GroupInput) else {
        return EXIT_MISSING_WITNESS;
    };

    verify::verify_artifacts(&preprocessing, &public_io, &proof)
}
