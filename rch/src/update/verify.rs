//! Checksum and signature verification.

use super::types::UpdateError;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

/// Result of verification.
#[derive(Debug)]
pub struct VerificationResult {
    pub checksum_valid: bool,
    pub signature_valid: Option<bool>,
}

/// Verify SHA256 checksum of a file.
pub async fn verify_checksum(
    file_path: &PathBuf,
    expected: &str,
) -> Result<VerificationResult, UpdateError> {
    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| UpdateError::InstallFailed(format!("Failed to open file: {}", e)))?;

    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .await
            .map_err(|e| UpdateError::InstallFailed(format!("Failed to read file: {}", e)))?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let actual = hasher.finalize().to_hex().to_string();

    // Also try SHA256 if BLAKE3 doesn't match (GitHub uses SHA256)
    let sha256_actual = compute_sha256(file_path).await?;

    // Check both BLAKE3 and SHA256
    let checksum_valid = actual.eq_ignore_ascii_case(expected)
        || sha256_actual.eq_ignore_ascii_case(expected);

    if !checksum_valid {
        return Err(UpdateError::ChecksumMismatch {
            expected: expected.to_string(),
            actual: sha256_actual,
        });
    }

    Ok(VerificationResult {
        checksum_valid: true,
        signature_valid: None, // TODO: Implement signature verification
    })
}

/// Compute SHA256 hash of a file.
async fn compute_sha256(file_path: &PathBuf) -> Result<String, UpdateError> {
    use std::io::Read;

    // Use blocking I/O wrapped in spawn_blocking for SHA256
    let path = file_path.clone();
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path)
            .map_err(|e| UpdateError::InstallFailed(format!("Failed to open file: {}", e)))?;

        let mut hasher = sha2::Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| UpdateError::InstallFailed(format!("Failed to read file: {}", e)))?;

            if bytes_read == 0 {
                break;
            }

            use sha2::Digest;
            hasher.update(&buffer[..bytes_read]);
        }

        use sha2::Digest;
        Ok(format!("{:x}", hasher.finalize()))
    })
    .await
    .map_err(|e| UpdateError::InstallFailed(format!("Task failed: {}", e)))?
}

/// Verify a byte slice against expected checksum.
pub fn verify_sha256_bytes(content: &[u8], expected: &str) -> Result<(), UpdateError> {
    use sha2::Digest;

    let mut hasher = sha2::Sha256::new();
    hasher.update(content);
    let actual = format!("{:x}", hasher.finalize());

    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(UpdateError::ChecksumMismatch {
            expected: expected.to_string(),
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_verify_sha256_bytes() {
        let content = b"test content";
        // SHA256 of "test content"
        let expected = "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72";

        assert!(verify_sha256_bytes(content, expected).is_ok());
    }

    #[test]
    fn test_verify_sha256_bytes_mismatch() {
        let content = b"test content";
        let wrong = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = verify_sha256_bytes(content, wrong);
        assert!(matches!(result, Err(UpdateError::ChecksumMismatch { .. })));
    }

    #[tokio::test]
    async fn test_compute_sha256() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        std::fs::write(&file_path, "test content").unwrap();

        let hash = compute_sha256(&file_path.to_path_buf()).await.unwrap();
        assert_eq!(
            hash,
            "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72"
        );
    }
}
