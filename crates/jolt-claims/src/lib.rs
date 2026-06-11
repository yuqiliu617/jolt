//! Shared claim and expression types for Jolt protocols.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

mod claims;
mod ops;
pub mod protocols;
mod util;

pub use claims::{
    challenge, constant, opening, public, ClaimExpression, ConsistencyClaim, Expr,
    InputClaimExpression, OutputClaimExpression, Source, Term,
};
