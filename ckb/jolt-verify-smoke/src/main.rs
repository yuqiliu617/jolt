//! Verifies exported Jolt artifacts using only the standalone verifier stack.
//!
//! This binary's dependency graph must never contain jolt-core, tracer, or
//! other prover/host machinery — `ckb/check-isolation.sh` enforces that.
//!
//! Reports live/peak heap statistics after verification: the peak is the
//! lower bound for sizing the ckb-vm contract heap.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use common::jolt_device::JoltDevice;
use jolt_crypto::{Bn254G1, Pedersen};
use jolt_dory::DoryScheme;
use jolt_field::Fr;
use jolt_transcript::Blake2bTranscript;
use jolt_verifier::{verify, JoltProof, JoltVerifierPreprocessing};

struct TrackingAlloc;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: delegates to System; only adds counters.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: TrackingAlloc = TrackingAlloc;

type Preprocessing = JoltVerifierPreprocessing<DoryScheme, Pedersen<Bn254G1>>;
type Proof = JoltProof<DoryScheme, Pedersen<Bn254G1>>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            #[expect(clippy::print_stderr, reason = "CLI error reporting")]
            {
                eprintln!("error: {e:#}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let dir: PathBuf = std::env::args_os()
        .nth(1)
        .context("usage: jolt-verify-smoke <artifact-dir>")?
        .into();

    let preprocessing: Preprocessing = read_artifact(&dir, jolt_artifacts::PREPROCESSING_FILE)?;
    let public_io: JoltDevice = read_artifact(&dir, jolt_artifacts::PUBLIC_IO_FILE)?;
    let proof: Proof = read_artifact(&dir, jolt_artifacts::PROOF_FILE)?;

    verify::<Fr, DoryScheme, Pedersen<Bn254G1>, Blake2bTranscript<Fr>>(
        &preprocessing,
        &public_io,
        &proof,
        None,
        false,
    )
    .map_err(|e| anyhow::anyhow!("verification failed: {e}"))?;

    #[expect(clippy::print_stdout, reason = "CLI success reporting")]
    {
        println!("proof verified OK");
        println!(
            "heap: live={} KiB, peak={} KiB",
            LIVE.load(Ordering::Relaxed) / 1024,
            PEAK.load(Ordering::Relaxed) / 1024,
        );
    }
    Ok(())
}

fn read_artifact<T: serde::de::DeserializeOwned>(dir: &std::path::Path, name: &str) -> Result<T> {
    let path = dir.join(name);
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    jolt_artifacts::decode(&bytes).with_context(|| format!("decode {}", path.display()))
}
