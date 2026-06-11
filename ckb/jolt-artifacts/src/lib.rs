//! Versioned container format for Jolt verifier artifacts.
//!
//! Each artifact is an 8-byte magic (`JVRT` + 4 version digits) followed by a
//! postcard-encoded payload. The magic guards against feeding mismatched or
//! stale artifacts to the verifier; bumping the format means bumping the
//! version digits.

use serde::{de::DeserializeOwned, Serialize};

pub const MAGIC: [u8; 8] = *b"JVRT0001";

pub const PREPROCESSING_FILE: &str = "preprocessing.bin";
pub const PUBLIC_IO_FILE: &str = "public_io.bin";
pub const PROOF_FILE: &str = "proof.bin";

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("artifact shorter than the {0}-byte header", MAGIC.len())]
    TooShort,
    #[error("artifact magic mismatch (wrong file or incompatible format version)")]
    BadMagic,
    #[error("postcard: {0}")]
    Postcard(#[from] postcard::Error),
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, ArtifactError> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend(postcard::to_allocvec(value)?);
    Ok(bytes)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ArtifactError> {
    let payload = bytes
        .strip_prefix(&MAGIC)
        .ok_or(if bytes.len() < MAGIC.len() {
            ArtifactError::TooShort
        } else {
            ArtifactError::BadMagic
        })?;
    Ok(postcard::from_bytes(payload)?)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "unwrap is fine in tests")]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let value = (42u64, "jolt".to_string());
        let bytes = encode(&value).unwrap();
        let decoded: (u64, String) = decode(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = encode(&7u32).unwrap();
        bytes[0] ^= 0xff;
        assert!(matches!(
            decode::<u32>(&bytes),
            Err(ArtifactError::BadMagic)
        ));
    }

    #[test]
    fn rejects_truncated_header() {
        assert!(matches!(
            decode::<u32>(&[0u8; 4]),
            Err(ArtifactError::TooShort)
        ));
    }
}
