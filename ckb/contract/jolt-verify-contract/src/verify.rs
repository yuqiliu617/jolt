//! Shared verification flow for the contract and bench entry points.

use common::jolt_device::JoltDevice;
use jolt_crypto::{Bn254G1, Pedersen};
use jolt_dory::DoryScheme;
use jolt_field::Fr;
use jolt_transcript::Blake2bTranscript;
use jolt_verifier::{verify, JoltProof, JoltVerifierPreprocessing};

pub type Preprocessing = JoltVerifierPreprocessing<DoryScheme, Pedersen<Bn254G1>>;
pub type Proof = JoltProof<DoryScheme, Pedersen<Bn254G1>>;

pub const EXIT_OK: i8 = 0;
pub const EXIT_BAD_PREPROCESSING: i8 = 10;
pub const EXIT_BAD_PUBLIC_IO: i8 = 11;
pub const EXIT_BAD_PROOF: i8 = 12;
pub const EXIT_REJECTED: i8 = 13;

/// Decodes the three artifacts and runs standard (non-ZK) verification.
pub fn verify_artifacts(
    preprocessing_bytes: &[u8],
    public_io_bytes: &[u8],
    proof_bytes: &[u8],
) -> i8 {
    let preprocessing: Preprocessing = match jolt_artifacts::decode(preprocessing_bytes) {
        Ok(p) => p,
        Err(_) => return EXIT_BAD_PREPROCESSING,
    };
    let public_io: JoltDevice = match jolt_artifacts::decode(public_io_bytes) {
        Ok(io) => io,
        Err(_) => return EXIT_BAD_PUBLIC_IO,
    };
    let proof: Proof = match jolt_artifacts::decode(proof_bytes) {
        Ok(p) => p,
        Err(_) => return EXIT_BAD_PROOF,
    };

    match verify::<Fr, DoryScheme, Pedersen<Bn254G1>, Blake2bTranscript<Fr>>(
        &preprocessing,
        &public_io,
        &proof,
        None,
        false,
    ) {
        Ok(()) => EXIT_OK,
        Err(_) => EXIT_REJECTED,
    }
}
