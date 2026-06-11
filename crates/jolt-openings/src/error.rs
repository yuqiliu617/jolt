//! PCS error types.

use alloc::string::String;
#[derive(Debug, thiserror::Error)]
pub enum OpeningsError {
    #[error("opening proof verification failed")]
    VerificationFailed,

    #[error("commitment mismatch: expected {expected}, got {actual}")]
    CommitmentMismatch { expected: String, actual: String },

    #[error("invalid setup parameters: {0}")]
    InvalidSetup(String),

    #[error("polynomial size {poly_size} exceeds setup max {setup_max}")]
    PolynomialTooLarge { poly_size: usize, setup_max: usize },
}
