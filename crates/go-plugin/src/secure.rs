//! Checksum verification for plugin binaries.
//!
//! Mirrors Go's `SecureConfig` -- hashes the plugin binary with SHA-256
//! and compares with a known checksum before launching.

use crate::error::PluginError;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Security configuration for verifying plugin binary integrity.
///
/// The host provides a known SHA-256 checksum. Before launching the plugin,
/// the binary is hashed and compared. This prevents executing tampered binaries.
#[derive(Debug, Clone)]
pub struct SecureConfig {
    /// Expected SHA-256 checksum of the plugin binary.
    pub checksum: [u8; 32],
}

impl SecureConfig {
    /// Create a new secure config from a hex-encoded checksum string.
    pub fn from_hex(hex: &str) -> Result<Self, PluginError> {
        let bytes = hex::decode(hex).map_err(|e| {
            PluginError::Other(format!("invalid checksum hex: {e}"))
        })?;
        if bytes.len() != 32 {
            return Err(PluginError::Other(format!(
                "checksum must be 32 bytes (SHA-256), got {}",
                bytes.len()
            )));
        }
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&bytes);
        Ok(Self { checksum })
    }

    /// Create a new secure config from raw bytes.
    pub fn from_bytes(checksum: [u8; 32]) -> Self {
        Self { checksum }
    }

    /// Verify that the plugin binary at `path` matches the expected checksum.
    ///
    /// Uses constant-time comparison to prevent timing attacks.
    pub fn check(&self, path: &Path) -> Result<(), PluginError> {
        let data = std::fs::read(path).map_err(|e| {
            PluginError::Subprocess(format!("failed to read plugin binary {:?}: {e}", path))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let actual: [u8; 32] = hasher.finalize().into();

        // Constant-time comparison
        if constant_time_eq(&actual, &self.checksum) {
            Ok(())
        } else {
            Err(PluginError::ChecksumMismatch)
        }
    }
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Compute the SHA-256 checksum of a file.
pub fn compute_checksum(path: &Path) -> Result<[u8; 32], PluginError> {
    let data = std::fs::read(path).map_err(|e| {
        PluginError::Subprocess(format!("failed to read file {:?}: {e}", path))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hasher.finalize().into())
}

/// Encode a checksum as a hex string.
pub fn checksum_hex(checksum: &[u8; 32]) -> String {
    hex::encode(checksum)
}

/// Simple hex encoding/decoding (avoids adding `hex` as a full dependency).
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("odd-length hex string".into());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| format!("invalid hex at position {i}: {e}"))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn checksum_verify_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plugin");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"test plugin binary content").unwrap();
        drop(f);

        let checksum = compute_checksum(&path).unwrap();
        let config = SecureConfig::from_bytes(checksum);
        assert!(config.check(&path).is_ok());
    }

    #[test]
    fn checksum_verify_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plugin");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"test plugin binary content").unwrap();
        drop(f);

        let config = SecureConfig::from_bytes([0u8; 32]);
        assert!(matches!(config.check(&path), Err(PluginError::ChecksumMismatch)));
    }

    #[test]
    fn from_hex_roundtrip() {
        let checksum = [42u8; 32];
        let hex_str = checksum_hex(&checksum);
        let config = SecureConfig::from_hex(&hex_str).unwrap();
        assert_eq!(config.checksum, checksum);
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }
}
