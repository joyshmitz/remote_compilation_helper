//! Download release artifacts with verification.

use super::check::UpdateCheck;
use super::types::{UpdateError, current_target};
use super::verify::verify_checksum;
use crate::ui::OutputContext;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// Progress callback for download updates.
#[allow(dead_code)]
pub type DownloadProgress = Box<dyn Fn(u64, u64) + Send>;

/// Result of a successful download.
#[allow(dead_code)]
pub struct DownloadedRelease {
    pub archive_path: PathBuf,
    pub checksum_verified: bool,
    pub version: String,
}

/// Download and verify a release.
pub async fn download_release(
    ctx: &OutputContext,
    update: &UpdateCheck,
) -> Result<DownloadedRelease, UpdateError> {
    // Find the asset for our platform
    let target = current_target();
    let archive_asset = update
        .assets
        .iter()
        .find(|a| a.name.contains(&target) && a.name.ends_with(".tar.gz"))
        .ok_or_else(|| UpdateError::UnsupportedPlatform(target.clone()))?;

    // Find the checksum file
    let checksum_asset = update
        .assets
        .iter()
        .find(|a| a.name == format!("{}.sha256", archive_asset.name))
        .or_else(|| update.assets.iter().find(|a| a.name == "checksums.txt"));

    if !ctx.is_json() {
        println!(
            "Downloading {} ({} bytes)...",
            archive_asset.name, archive_asset.size
        );
    }

    // Download to temp directory
    let temp_dir = std::env::temp_dir().join("rch-update");
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to create temp dir: {}", e)))?;

    let archive_path = temp_dir.join(&archive_asset.name);

    // Download with retries
    download_with_retry(&archive_asset.browser_download_url, &archive_path, 3).await?;

    // Verify checksum if available
    let checksum_verified = if let Some(checksum_asset) = checksum_asset {
        if !ctx.is_json() {
            println!("Verifying checksum...");
        }

        let checksum_path = temp_dir.join(&checksum_asset.name);
        download_with_retry(&checksum_asset.browser_download_url, &checksum_path, 3).await?;

        let expected_checksum = extract_checksum(&checksum_path, &archive_asset.name).await?;
        verify_checksum(&archive_path, &expected_checksum).await?;
        true
    } else {
        if !ctx.is_json() {
            println!("Warning: No checksum file available, skipping verification");
        }
        false
    };

    Ok(DownloadedRelease {
        archive_path,
        checksum_verified,
        version: update.latest_version.to_string(),
    })
}

/// Download a file with retry logic.
async fn download_with_retry(
    url: &str,
    dest: &PathBuf,
    max_retries: u32,
) -> Result<(), UpdateError> {
    let mut delay = Duration::from_secs(1);

    for attempt in 0..max_retries {
        match download_file(url, dest).await {
            Ok(()) => return Ok(()),
            Err(e) if is_transient_error(&e) => {
                if attempt + 1 < max_retries {
                    tracing::warn!(
                        "Download attempt {} failed: {}, retrying in {:?}",
                        attempt + 1,
                        e,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(Duration::from_secs(60));
                } else {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
    }

    Err(UpdateError::DownloadFailed(format!(
        "Failed after {} retries",
        max_retries
    )))
}

/// Download a single file.
async fn download_file(url: &str, dest: &PathBuf) -> Result<(), UpdateError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300)) // 5 minutes for large files
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| UpdateError::NetworkError(e.to_string()))?;

    let response = client
        .get(url)
        .header("User-Agent", format!("rch/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .await
        .map_err(|e| UpdateError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(UpdateError::DownloadFailed(format!(
            "Server returned {}",
            response.status()
        )));
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to create file: {}", e)))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| UpdateError::NetworkError(e.to_string()))?;

    file.write_all(&bytes)
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to write file: {}", e)))?;

    Ok(())
}

/// Check if an error is transient (worth retrying).
fn is_transient_error(e: &UpdateError) -> bool {
    matches!(e, UpdateError::NetworkError(_))
}

/// Extract checksum for a specific file from a checksum file.
async fn extract_checksum(checksum_file: &PathBuf, filename: &str) -> Result<String, UpdateError> {
    let content = tokio::fs::read_to_string(checksum_file)
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to read checksum file: {}", e)))?;

    // Checksum files typically have format: "checksum  filename" or "checksum filename"
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let checksum = parts[0];
            let file = parts.last().unwrap();
            if *file == filename || file.ends_with(filename) {
                return Ok(checksum.to_string());
            }
        } else if parts.len() == 1 && !content.contains(char::is_whitespace) {
            // Single checksum file for single asset
            return Ok(parts[0].to_string());
        }
    }

    Err(UpdateError::DownloadFailed(format!(
        "Checksum not found for {}",
        filename
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_extract_checksum_standard_format() {
        let temp = TempDir::new().unwrap();
        let checksum_file = temp.path().join("checksums.txt");

        std::fs::write(
            &checksum_file,
            "abc123  rch-v0.1.0-linux.tar.gz\ndef456  rch-v0.1.0-darwin.tar.gz",
        )
        .unwrap();

        let result =
            extract_checksum(&checksum_file.to_path_buf(), "rch-v0.1.0-linux.tar.gz").await;
        assert_eq!(result.unwrap(), "abc123");
    }

    #[tokio::test]
    async fn test_extract_checksum_not_found() {
        let temp = TempDir::new().unwrap();
        let checksum_file = temp.path().join("checksums.txt");

        std::fs::write(&checksum_file, "abc123  other-file.tar.gz").unwrap();

        let result =
            extract_checksum(&checksum_file.to_path_buf(), "rch-v0.1.0-linux.tar.gz").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_is_transient_error() {
        assert!(is_transient_error(&UpdateError::NetworkError(
            "timeout".to_string()
        )));
        assert!(!is_transient_error(&UpdateError::ChecksumMismatch {
            expected: "a".to_string(),
            actual: "b".to_string()
        }));
    }
}
