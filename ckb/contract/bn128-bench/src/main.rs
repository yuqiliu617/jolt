//! Same measurements as `jolt-prim-bench`, but on ckb-alt-bn128 (parity-bn
//! fork with riscv64-assembly Montgomery core) instead of arkworks. The delta
//! between the two is the headroom from swapping the field arithmetic backend.

#![no_std]
#![no_main]

use ckb_std::syscalls::{current_cycles, debug};
use ckb_std::{default_alloc, entry};
use parity_bn::{pairing, pairing_batch, Fr, Group, G1, G2};

entry!(main);
default_alloc!({ 4 * 1024 }, { 1024 * 1024 }, 64);

fn bench(name: &str, iters: u64, mut f: impl FnMut()) {
    let start = current_cycles();
    for _ in 0..iters {
        f();
    }
    let total = current_cycles() - start;
    debug(alloc::format!("prim {} {} {}", name, iters, total));
}

// Full-width (~254-bit) scalar: exponentiation costs scale with bit length,
// so narrow scalars would skew comparisons against the arkworks bench.
fn scalar(seed: u64) -> Fr {
    let mut x = Fr::from_str(
        "21888242871839275222246405745257275088548364400416034343698204186575808495615",
    )
    .unwrap();
    for _ in 0..=(seed % 4) {
        x = x * x + Fr::from_str("1234567891011121314151617").unwrap();
    }
    x
}

fn main() -> i8 {
    let s = scalar(42);
    let s2 = scalar(43);

    let g1 = G1::one() * s;
    let g1b = G1::one() * s2;
    let g2 = G2::one() * s;
    let gt = pairing(g1, g2);
    let gt2 = pairing(g1b, g2);

    let mut acc_fr = s;
    bench("fr_mul", 10_000, || {
        acc_fr = acc_fr * s2;
    });

    let mut acc_gt = gt;
    bench("gt_mul", 1_000, || {
        acc_gt = acc_gt * gt2;
    });
    bench("gt_exp", 20, || {
        acc_gt = acc_gt.pow(s);
    });

    let mut acc_g1 = g1;
    bench("g1_scalar_mul", 50, || {
        acc_g1 = acc_g1 * s;
    });
    let mut acc_g2 = g2;
    bench("g2_scalar_mul", 20, || {
        acc_g2 = acc_g2 * s;
    });

    bench("pairing_single", 10, || {
        acc_gt = acc_gt * pairing(acc_g1, g2);
    });
    let pairs = [(g1, g2), (g1b, g2), (acc_g1, g2), (g1, acc_g2)];
    bench("multi_pairing_4", 10, || {
        acc_gt = acc_gt * pairing_batch(&pairs);
    });

    debug(alloc::format!("sink {:?}", acc_gt == gt));
    0
}
