//! Verifies exported Jolt artifacts using only the standalone verifier stack.
//!
//! This binary's dependency graph must never contain jolt-core, tracer, or
//! other prover/host machinery — `ckb/check-isolation.sh` enforces that.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use common::jolt_device::JoltDevice;
use jolt_crypto::{Bn254G1, Pedersen};
use jolt_dory::DoryScheme;
use jolt_field::Fr;
use jolt_transcript::Blake2bTranscript;
use jolt_verifier::{verify, JoltProof, JoltVerifierPreprocessing};

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

    verify::<Fr, DoryScheme, Pedersen<Bn254G1>, Blake2bTranscript>(
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
    }
    Ok(())
}

fn read_artifact<T: serde::de::DeserializeOwned>(dir: &std::path::Path, name: &str) -> Result<T> {
    let path = dir.join(name);
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    jolt_artifacts::decode(&bytes).with_context(|| format!("decode {}", path.display()))
}
