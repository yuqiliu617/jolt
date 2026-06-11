//! Primitive micro-benchmarks inside ckb-vm.
//!
//! Measures the BN254 building blocks the verifier spends its cycles on, via
//! the `current_cycles` syscall. Output: `prim <name> <iters> <total-cycles>`.

#![no_std]
#![no_main]

mod sync_shims;

use ckb_std::syscalls::{current_cycles, debug};
use ckb_std::{default_alloc, entry};
use jolt_crypto::{Bn254, Bn254G1, Bn254G2, Bn254GT, JoltGroup, PairingGroup};
use jolt_field::{Fr, FromPrimitiveInt};
use jolt_transcript::{AppendToTranscript, Blake2bTranscript, Transcript};

entry!(main);
default_alloc!({ 4 * 1024 }, { 2304 * 1024 }, 64);

fn bench(name: &str, iters: u64, mut f: impl FnMut()) {
    let start = current_cycles();
    for _ in 0..iters {
        f();
    }
    let total = current_cycles() - start;
    debug(alloc::format!("prim {} {} {}", name, iters, total));
}

// Non-trivial deterministic scalar (no RNG in the VM).
fn scalar(seed: u64) -> Fr {
    let mut x = Fr::from_u64(seed ^ 0x9e37_79b9_7f4a_7c15);
    for _ in 0..4 {
        x = x * x + Fr::from_u64(seed);
    }
    x
}

fn main() -> i8 {
    let s = scalar(42);
    let s2 = scalar(43);

    let g1 = Bn254::g1_generator().scalar_mul(&s);
    let g1b = Bn254::g1_generator().scalar_mul(&s2);
    let g2 = Bn254::g2_generator().scalar_mul(&s);
    let gt = Bn254::pairing(&g1, &g2);
    let gt2 = Bn254::pairing(&g1b, &g2);

    // Field ops (high iteration counts; loop overhead is negligible at 1-cycle
    // adds vs hundreds for a field mul).
    let mut acc_fr = s;
    bench("fr_mul", 10_000, || {
        acc_fr = acc_fr * s2;
    });

    // GT (Fq12) group ops. `add` on Bn254GT is Fq12 multiplication.
    let mut acc_gt = gt;
    bench("gt_mul", 1_000, || {
        acc_gt = acc_gt + gt2;
    });
    bench("gt_exp", 20, || {
        acc_gt = acc_gt.scalar_mul(&s);
    });

    // G1/G2 scalar muls.
    let mut acc_g1 = g1;
    bench("g1_scalar_mul", 50, || {
        acc_g1 = acc_g1.scalar_mul(&s);
    });
    let mut acc_g2 = g2;
    bench("g2_scalar_mul", 20, || {
        acc_g2 = acc_g2.scalar_mul(&s);
    });

    // Pairings.
    bench("pairing_single", 10, || {
        acc_gt = acc_gt + Bn254::pairing(&acc_g1, &g2);
    });
    let g1s = [g1, g1b, acc_g1, g1];
    let g2s = [g2, g2, g2, acc_g2];
    bench("multi_pairing_4", 10, || {
        acc_gt = acc_gt + Bn254::multi_pairing(&g1s, &g2s);
    });

    // GT serde: deserialization enforces the r-torsion subgroup check
    // (x^r == 1), the suspected decode_proof dominator.
    let gt_bytes = postcard_roundtrip_bytes(&gt);
    bench("gt_deser_checked", 10, || {
        let parsed: Bn254GT = postcard::from_bytes(&gt_bytes).unwrap();
        acc_gt = acc_gt + parsed;
    });

    // Transcript absorb+challenge (Blake2b), per-round sumcheck cost.
    let mut transcript = Blake2bTranscript::<Fr>::new(b"bench");
    bench("transcript_round", 100, || {
        acc_fr.append_to_transcript(&mut transcript);
        acc_fr = transcript.challenge();
    });

    // Keep results live so nothing is optimized away.
    let sink = (acc_fr, acc_g1, acc_g2, acc_gt);
    debug(alloc::format!("sink {:?}", sink.0));
    0
}

fn postcard_roundtrip_bytes(gt: &Bn254GT) -> alloc::vec::Vec<u8> {
    postcard::to_allocvec(gt).unwrap()
}
