//! Proves a guest program with jolt-core, converts the proof and verifier
//! preprocessing into the standalone `jolt-verifier` model, self-checks both
//! verifiers, and writes postcard artifacts for VM-side consumption.

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use common::jolt_device::JoltDevice;
use jolt_core::host;
use jolt_core::zkvm::prover::JoltProverPreprocessing;
use jolt_core::zkvm::verifier::{
    JoltSharedPreprocessing, JoltVerifierPreprocessing as CoreVerifierPreprocessing,
};
use jolt_core::zkvm::{RV64IMACProof, RV64IMACProver, RV64IMACVerifier, Serializable};
use jolt_crypto::{Bn254G1, Pedersen};
use jolt_dory::DoryScheme;
use jolt_field::Fr;
use jolt_transcript::Blake2bTranscript;
use jolt_verifier::compat::convert::{convert_core_preprocessing, ImportedCoreProof};
use jolt_verifier::JoltVerifierPreprocessing;

type CoreField = jolt_core::ark_bn254::Fr;
type CoreCurve = jolt_core::curve::Bn254Curve;
type CorePcs = jolt_core::poly::commitment::dory::DoryCommitmentScheme;
type ModelPreprocessing = JoltVerifierPreprocessing<DoryScheme, Pedersen<Bn254G1>>;
type ModelProof = ImportedCoreProof<CoreField, CoreCurve, CorePcs>;

#[derive(Parser)]
#[command(about = "Prove a guest and export jolt-verifier artifacts")]
struct Args {
    /// Guest preset to prove
    #[arg(long, value_parser = ["fibonacci", "muldiv"])]
    guest: String,
    /// Output directory for artifacts
    #[arg(long, default_value = "target/jolt-artifacts")]
    out: PathBuf,
}

struct GuestPreset {
    package: &'static str,
    inputs: Vec<u8>,
}

fn preset(name: &str) -> Result<GuestPreset> {
    match name {
        "fibonacci" => Ok(GuestPreset {
            package: "fibonacci-guest",
            inputs: postcard::to_stdvec(&100u32)?,
        }),
        "muldiv" => Ok(GuestPreset {
            package: "muldiv-guest",
            inputs: postcard::to_stdvec(&[9u32, 5u32, 3u32])?,
        }),
        other => bail!("unknown guest preset: {other}"),
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let preset = preset(&args.guest)?;

    let (model_preprocessing, public_io, model_proof) = prove_and_convert(&preset)?;

    jolt_verifier::verify::<Fr, DoryScheme, Pedersen<Bn254G1>, Blake2bTranscript<Fr>>(
        &model_preprocessing,
        &public_io,
        &model_proof,
        None,
        false,
    )
    .map_err(|e| anyhow!("self-check: jolt-verifier rejected the converted proof: {e}"))?;

    fs::create_dir_all(&args.out)
        .with_context(|| format!("create output dir {}", args.out.display()))?;
    write_artifact(
        &args.out,
        jolt_artifacts::PREPROCESSING_FILE,
        &model_preprocessing,
    )?;
    write_artifact(&args.out, jolt_artifacts::PUBLIC_IO_FILE, &public_io)?;
    write_artifact(&args.out, jolt_artifacts::PROOF_FILE, &model_proof)?;

    Ok(())
}

fn prove_and_convert(preset: &GuestPreset) -> Result<(ModelPreprocessing, JoltDevice, ModelProof)> {
    let mut program = host::Program::new(preset.package);
    let (bytecode, init_memory_state, _, entry_address) = program.decode();
    let (_, _, _, trace_io) = program.trace(&preset.inputs, &[], &[]);

    let shared = JoltSharedPreprocessing::new(
        bytecode,
        trace_io.memory_layout.clone(),
        init_memory_state,
        1 << 16,
        entry_address,
    )
    .map_err(|e| anyhow!("shared preprocessing failed: {e}"))?;
    let prover_preprocessing =
        JoltProverPreprocessing::<CoreField, CoreCurve, CorePcs>::new(shared);

    let elf = program
        .get_elf_contents()
        .context("guest ELF missing after decode")?;
    let prover = RV64IMACProver::gen_from_elf(
        &prover_preprocessing,
        &elf,
        &preset.inputs,
        &[],
        &[],
        None,
        None,
        None,
    );
    let public_io = prover.program_io.clone();
    let (proof, _) = prover.prove();

    let core_preprocessing = CoreVerifierPreprocessing::from(&prover_preprocessing);
    let proof_bytes = proof
        .serialize_to_bytes()
        .map_err(|e| anyhow!("serialize core proof: {e}"))?;
    let proof_copy = RV64IMACProof::deserialize_from_bytes(&proof_bytes)
        .map_err(|e| anyhow!("roundtrip core proof: {e}"))?;
    RV64IMACVerifier::new(
        &core_preprocessing,
        proof_copy,
        public_io.clone(),
        None,
        None,
    )
    .and_then(RV64IMACVerifier::verify)
    .map_err(|e| anyhow!("self-check: jolt-core verifier rejected the proof: {e}"))?;

    let model_preprocessing = convert_core_preprocessing(&core_preprocessing)
        .map_err(|e| anyhow!("preprocessing conversion failed: {e}"))?;
    let model_proof =
        ModelProof::try_from(proof).map_err(|e| anyhow!("proof conversion failed: {e}"))?;

    Ok((model_preprocessing, public_io, model_proof))
}

fn write_artifact<T: serde::Serialize>(dir: &std::path::Path, name: &str, value: &T) -> Result<()> {
    let path = dir.join(name);
    let bytes = jolt_artifacts::encode(value)?;
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
