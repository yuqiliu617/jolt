//! no_std compile gate for the Jolt verifier stack on bare-metal riscv64.
//!
//! Checked with `cargo check -p jolt-verify-nostd-check --target
//! riscv64imac-unknown-none-elf`; any std leakage in the dependency closure
//! fails this build. Mirrors the flow of `jolt-verify-smoke`: decode postcard
//! artifacts, run the full standard verification.

#![no_std]

extern crate alloc;

use common::jolt_device::JoltDevice;
use jolt_crypto::{Bn254G1, Pedersen};
use jolt_dory::DoryScheme;
use jolt_field::Fr;
use jolt_transcript::Blake2bTranscript;
use jolt_verifier::{verify, JoltProof, JoltVerifierPreprocessing, VerifierError};

type Preprocessing = JoltVerifierPreprocessing<DoryScheme, Pedersen<Bn254G1>>;
type Proof = JoltProof<DoryScheme, Pedersen<Bn254G1>>;

#[derive(Debug)]
pub enum VerifyArtifactsError {
    Artifact(jolt_artifacts::ArtifactError),
    Verification(VerifierError),
}

/// Decodes the three verifier artifacts and runs standard (non-ZK) verification.
pub fn verify_artifacts(
    preprocessing_bytes: &[u8],
    public_io_bytes: &[u8],
    proof_bytes: &[u8],
) -> Result<(), VerifyArtifactsError> {
    let preprocessing: Preprocessing =
        jolt_artifacts::decode(preprocessing_bytes).map_err(VerifyArtifactsError::Artifact)?;
    let public_io: JoltDevice =
        jolt_artifacts::decode(public_io_bytes).map_err(VerifyArtifactsError::Artifact)?;
    let proof: Proof =
        jolt_artifacts::decode(proof_bytes).map_err(VerifyArtifactsError::Artifact)?;

    verify::<Fr, DoryScheme, Pedersen<Bn254G1>, Blake2bTranscript<Fr>>(
        &preprocessing,
        &public_io,
        &proof,
        None,
        false,
    )
    .map_err(VerifyArtifactsError::Verification)
}
