// TERMIN.AI: Environment variable loader with security checks
//
// Loads environment variables from terminai.env in the config directory.
// Includes security checks to ensure the file is not world-readable.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// Get the path to the terminai.env file in the config directory
fn get_env_file_path() -> PathBuf {
  let xdg_dirs = xdg::BaseDirectories::with_prefix("terminai");
  xdg_dirs
    .get_config_home()
    .expect("Failed to determine home directory")
    .join("terminai.env")
}

/// Check if a file is world-readable (insecure for API keys)
#[cfg(unix)]
fn is_world_readable(path: &std::path::Path) -> Result<bool> {
  use std::os::unix::fs::PermissionsExt;

  let metadata = std::fs::metadata(path).with_context(|| {
    format!("Failed to read metadata for {}", path.display())
  })?;

  let permissions = metadata.permissions();
  let mode = permissions.mode();

  // Check if "other" has read permission (world-readable)
  // In Unix permissions, the last 3 bits are: read (4), write (2), execute (1)
  // We check if bit 2 (read for "other") is set
  Ok((mode & 0o004) != 0)
}

#[cfg(not(unix))]
fn is_world_readable(_path: &std::path::Path) -> Result<bool> {
  // On non-Unix systems, we can't reliably check world-readable permissions
  // We'll just return false (assume it's secure) and rely on OS file permissions
  log::warn!(
    "Cannot check file permissions on non-Unix systems. \
     Ensure terminai.env is not world-readable."
  );
  Ok(false)
}

/// Load environment variables from terminai.env in the config directory
///
/// This function:
/// - Locates the terminai.env file in the XDG config directory
/// - Checks that the file is NOT world-readable (security requirement)
/// - Loads environment variables from the file if it exists
/// - Silently succeeds if the file doesn't exist
///
/// # Errors
///
/// Returns an error if:
/// - The file exists but is world-readable (security violation)
/// - The file exists but cannot be read
/// - The file exists but contains invalid syntax
pub fn load_env_file() -> Result<()> {
  let env_path = get_env_file_path();

  // If the file doesn't exist, that's fine - just return
  if !env_path.exists() {
    log::debug!("No terminai.env file found at {}", env_path.display());
    return Ok(());
  }

  log::info!("Found terminai.env file at {}", env_path.display());

  // Security check: ensure the file is NOT world-readable
  if is_world_readable(&env_path)? {
    bail!(
      "SECURITY ERROR: {} is world-readable!\n\
       This file may contain API keys and must not be readable by other users.\n\
       Fix with: chmod 600 {}",
      env_path.display(),
      env_path.display()
    );
  }

  // Load the environment variables from the file
  dotenvy::from_path(&env_path).with_context(|| {
    format!(
      "Failed to load environment variables from {}",
      env_path.display()
    )
  })?;

  log::info!("Loaded environment variables from {}", env_path.display());

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  #[test]
  fn test_get_env_file_path() {
    let path = get_env_file_path();
    assert!(path.ends_with("terminai.env"));
  }

  #[cfg(unix)]
  #[test]
  fn test_world_readable_check() {
    use std::os::unix::fs::PermissionsExt;

    // Create a temporary file
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("test_terminai_permissions.env");

    // Write some content
    {
      let mut file = std::fs::File::create(&test_file).unwrap();
      file.write_all(b"TEST_KEY=test_value\n").unwrap();
    }

    // Test 1: Set permissions to 644 (world-readable)
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(
      is_world_readable(&test_file).unwrap(),
      "File with 644 should be world-readable"
    );

    // Test 2: Set permissions to 600 (not world-readable)
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(
      !is_world_readable(&test_file).unwrap(),
      "File with 600 should not be world-readable"
    );

    // Test 3: Set permissions to 640 (not world-readable)
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    assert!(
      !is_world_readable(&test_file).unwrap(),
      "File with 640 should not be world-readable"
    );

    // Cleanup
    std::fs::remove_file(test_file).ok();
  }
}
