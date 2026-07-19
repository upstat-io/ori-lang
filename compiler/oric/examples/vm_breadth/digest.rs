//! SHA-256 and byte-preserving artifact encoding.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Exact bytes or a bounded prefix with a digest over the complete stream.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "encoding", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum EncodedBytes {
    /// Every byte is retained as lowercase hexadecimal.
    CompleteHex { hex: String },
    /// Only a prefix is retained; the length and digest still cover all bytes.
    PrefixHex {
        hex: String,
        retained_length_decimal: String,
    },
}

/// Length, SHA-256 digest, and byte-preserving representation of an artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ByteArtifact {
    pub(crate) length_decimal: String,
    pub(crate) sha256: String,
    pub(crate) bytes: EncodedBytes,
}

impl ByteArtifact {
    pub(crate) fn complete(bytes: &[u8]) -> Self {
        Self {
            length_decimal: bytes.len().to_string(),
            sha256: sha256(bytes),
            bytes: EncodedBytes::CompleteHex { hex: to_hex(bytes) },
        }
    }

    pub(crate) fn captured(total_length: u64, digest: [u8; 32], retained: &[u8]) -> Self {
        let bytes = match usize::try_from(total_length) {
            Ok(length) if length == retained.len() => EncodedBytes::CompleteHex {
                hex: to_hex(retained),
            },
            _ => EncodedBytes::PrefixHex {
                hex: to_hex(retained),
                retained_length_decimal: retained.len().to_string(),
            },
        };
        Self {
            length_decimal: total_length.to_string(),
            sha256: to_hex(&digest),
            bytes,
        }
    }
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    to_hex(&digest)
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
