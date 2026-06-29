#![cfg_attr(feature = "guest", no_std)]

extern crate alloc;

use ckb_vote_types::molecules::{
    blockchain,
    types::{BlockVecReader, GuestProgramArgumentsReader, PublicValues},
    verify_block_vec,
};
use molecule::prelude::{Builder, Entity, Reader};

/// Verifies the CKB vote PoC over the supplied `GuestProgramArguments` molecule
/// blob — identical computation to the SP1 guest
/// (`ckb-vote-poc/sp1/ckb-vote-verification/program/src/main.rs`): structural
/// molecule check, block-integrity (parent-hash chain + blake2b CBMT
/// transactions-root), and vote tally. Returns the blake2b-256 digest of the
/// `PublicValues` the SP1 guest commits, binding the full result in a small
/// output.
#[jolt::provable(
    max_input_size = 2_097_152,
    max_output_size = 256,
    stack_size = 1_048_576,
    heap_size = 67_108_864,
    max_trace_length = 4_194_304
)]
fn vote(data: &[u8]) -> [u8; 32] {
    let args = GuestProgramArgumentsReader::from_slice(data)
        .expect("failed to load guest program arguments");

    let blocks_bytes = args.blocks().raw_data();
    verify_block_vec(blocks_bytes, false).expect("invalid BlockVec molecule");
    let blocks = BlockVecReader::new_unchecked(blocks_bytes);
    let witness_root = args.witness_root();

    ckb_vote_verification::verify_block_integrity(blocks, witness_root)
        .expect("block integrity verification failed");

    let first_block = blocks.get(0).expect("at least one block");
    let start_hash = ckb_vote_verification::compute_header_hash(first_block.header());
    let last_idx = blocks.len().saturating_sub(1);
    let last_block = blocks.get(last_idx).expect("last block exists");
    let end_hash = ckb_vote_verification::compute_header_hash(last_block.header());

    let result = ckb_vote_verification::count_vote(blocks, args.proposal_script().to_entity());

    let public_values = PublicValues::new_builder()
        .proposal(result.proposal)
        .start_block_hash(blockchain::Byte32::from(start_hash))
        .end_block_hash(blockchain::Byte32::from(end_hash))
        .proposal_script(args.proposal_script().to_entity())
        .passed(blockchain::Byte::from(u8::from(result.passed)))
        .yes_vote(blockchain::Uint64::from(result.yes_vote.to_le_bytes()))
        .no_vote(blockchain::Uint64::from(result.no_vote.to_le_bytes()))
        .build();

    ckb_vote_verification::blake2b_256(public_values.as_slice())
}
