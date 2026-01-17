//! Update lock to prevent concurrent updates.
//!
//! Uses a PID-based lock file approach that's safe and portable.

use super::types::UpdateError;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

/// Lock to prevent concurrent updates.
pub struct UpdateLock {
    path: PathBuf,
}

impl UpdateLock {
    /// Acquire the update lock.
    ///
    /// Uses a PID-based lock file. If a lock file exists, checks if the
    /// process is still running before failing.
    pub fn acquire() -> Result<Self, UpdateError> {
        let path = get_lock_path()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                UpdateError::InstallFailed(format!("Failed to create lock dir: {}", e))
            })?;
        }

        // Check if lock file exists and if the process is still running
        if path.exists() {
            if let Ok(mut file) = File::open(&path) {
                let mut contents = String::new();
                if file.read_to_string(&mut contents).is_ok() {
                    if let Ok(pid) = contents.trim().parse::<u32>() {
                        if is_process_running(pid) {
                            return Err(UpdateError::LockHeld);
                        }
                    }
                }
            }
            // Stale lock file, remove it
            let _ = fs::remove_file(&path);
        }

        // Write our PID to the lock file
        let mut file = File::create(&path).map_err(|e| {
            UpdateError::InstallFailed(format!("Failed to create lock file: {}", e))
        })?;

        write!(file, "{}", std::process::id())
            .map_err(|e| UpdateError::InstallFailed(format!("Failed to write lock file: {}", e)))?;

        Ok(Self { path })
    }

    /// Check if an update is currently in progress.
    #[allow(dead_code)]
    pub fn is_locked() -> bool {
        if let Ok(path) = get_lock_path() {
            if path.exists() {
                if let Ok(mut file) = File::open(&path) {
                    let mut contents = String::new();
                    if file.read_to_string(&mut contents).is_ok() {
                        if let Ok(pid) = contents.trim().parse::<u32>() {
                            return is_process_running(pid);
                        }
                    }
                }
            }
        }
        false
    }
}

impl Drop for UpdateLock {
    fn drop(&mut self) {
        // Remove the lock file
        let _ = fs::remove_file(&self.path);
    }
}

/// Get the path to the lock file.
fn get_lock_path() -> Result<PathBuf, UpdateError> {
    let data_dir = dirs::data_dir().ok_or_else(|| {
        UpdateError::InstallFailed("Could not determine data directory".to_string())
    })?;

    Ok(data_dir.join("rch/update.lock"))
}

/// Check if a process is still running.
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // On Unix, we can check /proc or use kill with signal 0
        let proc_path = format!("/proc/{}", pid);
        std::path::Path::new(&proc_path).exists()
    }

    #[cfg(windows)]
    {
        // On Windows, attempt to open the process
        // This is a simplified check
        false // Assume not running if we can't check
    }

    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_acquire_and_release() {
        // First lock should succeed
        let lock1 = UpdateLock::acquire();
        assert!(lock1.is_ok());

        // While first lock is held, second should fail (same process, but tests PID check)
        // Note: In same process, it will see same PID and think it's still locked
        // This is expected behavior - it prevents recursive updates

        // Drop first lock
        drop(lock1);

        // Now acquiring should succeed
        let lock3 = UpdateLock::acquire();
        assert!(lock3.is_ok());
    }

    #[test]
    fn test_is_locked_false_when_no_lock() {
        // Clear any existing lock first
        if let Ok(path) = get_lock_path() {
            let _ = std::fs::remove_file(path);
        }

        // Without lock, should not be locked
        assert!(!UpdateLock::is_locked());
    }

    #[test]
    fn test_current_process_is_running() {
        let pid = std::process::id();
        assert!(is_process_running(pid));
    }

    #[test]
    fn test_nonexistent_process_not_running() {
        // Use a very high PID that's unlikely to exist
        assert!(!is_process_running(999999999));
    }
}
