//! Proves a guest program with jolt-core, converts the proof and verifier
//! preprocessing into the standalone `jolt-verifier` model, self-checks both
//! verifiers, and writes postcard artifacts for VM-side consumption.

#![expect(clippy::print_stdout, reason = "CLI tool reports progress to stdout")]

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

const MB: u64 = 1024 * 1024;

#[derive(Parser)]
#[command(about = "Prove a guest and export jolt-verifier artifacts")]
struct Args {
    /// Guest preset to prove
    #[arg(long, value_parser = ["fibonacci", "muldiv", "vote"])]
    guest: String,
    /// Output directory for artifacts
    #[arg(long, default_value = "target/jolt-artifacts")]
    out: PathBuf,
    /// Build + trace the guest, print the cycle count and padded trace length,
    /// then exit (no proving). Use to size the proof before the slow prove.
    #[arg(long)]
    trace_only: bool,
}

struct GuestPreset {
    package: &'static str,
    inputs: Vec<u8>,
    heap_size: u64,
    stack_size: u64,
    max_input_size: u64,
    /// Must equal the guest's `#[jolt::provable(max_output_size = …)]`. The I/O
    /// region size is baked into the compiled guest; a host/guest mismatch
    /// misplaces the output region and fails memory-checking. `None` keeps the
    /// host default (which the fibonacci/muldiv guests rely on).
    max_output_size: Option<u64>,
}

fn preset(name: &str) -> Result<GuestPreset> {
    match name {
        "fibonacci" => Ok(GuestPreset {
            package: "fibonacci-guest",
            inputs: postcard::to_stdvec(&100u32)?,
            heap_size: 32 * MB,
            stack_size: 4096,
            max_input_size: 4096,
            max_output_size: None,
        }),
        "muldiv" => Ok(GuestPreset {
            package: "muldiv-guest",
            inputs: postcard::to_stdvec(&[9u32, 5u32, 3u32])?,
            heap_size: 32 * MB,
            stack_size: 4096,
            max_input_size: 4096,
            max_output_size: None,
        }),
        "vote" => Ok(GuestPreset {
            package: "ckb-vote-guest",
            inputs: vote_input()?,
            heap_size: 64 * MB,
            stack_size: MB,
            max_input_size: 2 * MB,
            max_output_size: Some(256),
        }),
        other => bail!("unknown guest preset: {other}"),
    }
}

/// Builds the `vote` guest input identically to the SP1 PoC `--mock` path:
/// `ckb_vote_testtool::generate_from_templates(sample_proposal(), blocks.bin)`,
/// postcard-encoded for the guest's `&[u8]` argument.
///
/// The full 5-block template traces to ~2^26 cycles, whose proof needs >2 GB
/// contiguous polynomial allocations. Set `VOTE_BLOCKS=N` to prove the first N
/// blocks instead (the on-chain verify cost is workload-independent, so a
/// shorter trace yields the same comparison at a provable size).
fn vote_input() -> Result<Vec<u8>> {
    use ckb_vote_types::molecules::{
        blockchain,
        types::{BlockVec, Proposal},
    };
    use molecule::prelude::{Builder, Entity};

    let blocks_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../ckb-vote-poc/crates/verification/tests/blocks.bin");
    let mut block_data = fs::read(&blocks_path)
        .with_context(|| format!("read vote blocks {}", blocks_path.display()))?;

    if let Ok(spec) = std::env::var("VOTE_BLOCKS") {
        let n: usize = spec
            .parse()
            .with_context(|| format!("VOTE_BLOCKS must be a number, got {spec:?}"))?;
        let full = BlockVec::from_compatible_slice(&block_data)
            .map_err(|e| anyhow!("parse template BlockVec: {e:?}"))?;
        let take = n.min(full.len());
        let mut builder = BlockVec::new_builder();
        for i in 0..take {
            if let Some(block) = full.get(i) {
                builder = builder.push(block);
            }
        }
        block_data = builder.build().as_slice().to_vec();
    }

    // Mirrors sample_proposal() in the SP1 script.
    let proposal = Proposal::new_builder()
        .vote_cell_code_hash(blockchain::Byte32::from([1u8; 32]))
        .vote_cell_hash_type(blockchain::Byte::new(0))
        .minimal_requirement(blockchain::Uint64::from(0u64.to_le_bytes()))
        .build();

    let guest_args = ckb_vote_testtool::generate_from_templates(proposal, &block_data)
        .map_err(|e| anyhow!("generate_from_templates: {e:?}"))?;

    Ok(postcard::to_stdvec(&guest_args.as_slice().to_vec())?)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let preset = preset(&args.guest)?;

    if args.trace_only {
        let mut program = configured_program(&preset);
        let _ = program.decode();
        let (_, trace, _, io) = program.trace(&preset.inputs, &[], &[]);
        let padded = padded_trace_length(trace.len());
        println!(
            "guest {} traced {} cycles -> padded trace length {}",
            preset.package,
            trace.len(),
            padded
        );
        println!(
            "guest panic={} input={}B output={}B",
            io.panic,
            io.inputs.len(),
            io.outputs.len()
        );
        return Ok(());
    }

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

fn configured_program(preset: &GuestPreset) -> host::Program {
    let mut program = host::Program::new(preset.package);
    program.set_heap_size(preset.heap_size);
    program.set_stack_size(preset.stack_size);
    program.set_max_input_size(preset.max_input_size);
    if let Some(max_output_size) = preset.max_output_size {
        program.set_max_output_size(max_output_size);
    }
    program
}

/// Dory needs a power-of-two padded trace length ≥ the executed cycle count.
/// Floor at 2^16 to match the original behaviour for small guests.
fn padded_trace_length(cycles: usize) -> usize {
    cycles.next_power_of_two().max(1 << 16)
}

fn prove_and_convert(preset: &GuestPreset) -> Result<(ModelPreprocessing, JoltDevice, ModelProof)> {
    let mut program = configured_program(preset);
    let (bytecode, init_memory_state, _, entry_address) = program.decode();
    let (_, trace, _, trace_io) = program.trace(&preset.inputs, &[], &[]);
    let padded = padded_trace_length(trace.len());
    println!(
        "guest {} traced {} cycles -> padded trace length {}",
        preset.package,
        trace.len(),
        padded
    );

    let shared = JoltSharedPreprocessing::new(
        bytecode,
        trace_io.memory_layout.clone(),
        init_memory_state,
        padded,
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
