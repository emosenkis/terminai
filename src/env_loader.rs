// Terminai: Environment variable loader with security checks
//
// Loads environment variables from terminai.env in the config directory.
// Includes security checks to ensure the file is not group or world-readable.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// Get the path to the terminai.env file in the config directory
pub fn env_file_path() -> PathBuf {
  crate::paths::config_dir()
    .expect("Failed to determine Terminai config directory")
    .join("terminai.env")
}

/// Check if a file has insecure permissions (group or world readable)
#[cfg(unix)]
fn has_insecure_permissions(path: &std::path::Path) -> Result<bool> {
  use std::os::unix::fs::PermissionsExt;

  let metadata = std::fs::metadata(path).with_context(|| {
    format!("Failed to read metadata for {}", path.display())
  })?;

  let permissions = metadata.permissions();
  let mode = permissions.mode();

  // Check if "group" or "other" has read permission
  // Unix permissions: owner(rwx) group(rwx) other(rwx)
  // Group read: 0o040 (bit 5)
  // Other read: 0o004 (bit 2)
  // File must be 600 (or 400) - only owner can read
  Ok((mode & 0o044) != 0)
}

#[cfg(not(unix))]
fn has_insecure_permissions(_path: &std::path::Path) -> Result<bool> {
  // On non-Unix systems, we can't reliably check file permissions
  // We'll just return false (assume it's secure) and rely on OS file permissions
  log::warn!(
    "Cannot check file permissions on non-Unix systems. \
     Ensure terminai.env has appropriate permissions (owner-only readable)."
  );
  Ok(false)
}

/// Load environment variables from terminai.env in the config directory
///
/// This function:
/// - Locates the terminai.env file in the XDG config directory
/// - Checks that the file is NOT group or world-readable (security requirement)
/// - Loads environment variables from the file if it exists
/// - Silently succeeds if the file doesn't exist
///
/// # Errors
///
/// Returns an error if:
/// - The file exists but is group or world-readable (security violation)
/// - The file exists but cannot be read
/// - The file exists but contains invalid syntax
pub fn load_env_file() -> Result<()> {
  load_env_file_with_override(false)
}

pub fn reload_env_file() -> Result<()> {
  load_env_file_with_override(true)
}

fn load_env_file_with_override(override_existing: bool) -> Result<()> {
  let env_path = env_file_path();

  // If the file doesn't exist, that's fine - just return
  if !env_path.exists() {
    log::debug!("No terminai.env file found at {}", env_path.display());
    return Ok(());
  }

  log::info!("Found terminai.env file at {}", env_path.display());

  // Security check: ensure the file is NOT group or world-readable
  if has_insecure_permissions(&env_path)? {
    bail!(
      "SECURITY ERROR: {} has insecure permissions!\n\
       This file may contain API keys and must only be readable by the owner.\n\
       Fix with: chmod 600 {}",
      env_path.display(),
      env_path.display()
    );
  }

  // Load the environment variables from the file
  if override_existing {
    dotenvy::from_path_override(&env_path).with_context(|| {
      format!(
        "Failed to reload environment variables from {}",
        env_path.display()
      )
    })?;
  } else {
    dotenvy::from_path(&env_path).with_context(|| {
      format!(
        "Failed to load environment variables from {}",
        env_path.display()
      )
    })?;
  }

  log::info!("Loaded environment variables from {}", env_path.display());

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  #[test]
  fn test_get_env_file_path() {
    let path = env_file_path();
    assert!(path.ends_with("terminai.env"));
  }

  #[cfg(unix)]
  #[test]
  fn test_permission_checks() {
    use std::os::unix::fs::PermissionsExt;

    // Create a temporary file
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("test_terminai_permissions.env");

    // Write some content
    {
      let mut file = std::fs::File::create(&test_file).unwrap();
      file.write_all(b"TEST_KEY=test_value\n").unwrap();
    }

    // Test 1: 644 (group + world readable) - INSECURE
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(
      has_insecure_permissions(&test_file).unwrap(),
      "File with 644 should be insecure (group + world readable)"
    );

    // Test 2: 640 (group readable) - INSECURE
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    assert!(
      has_insecure_permissions(&test_file).unwrap(),
      "File with 640 should be insecure (group readable)"
    );

    // Test 3: 604 (world readable) - INSECURE
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o604),
    )
    .unwrap();
    assert!(
      has_insecure_permissions(&test_file).unwrap(),
      "File with 604 should be insecure (world readable)"
    );

    // Test 4: 600 (owner only) - SECURE
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(
      !has_insecure_permissions(&test_file).unwrap(),
      "File with 600 should be secure (owner read/write only)"
    );

    // Test 5: 400 (owner read-only) - SECURE
    std::fs::set_permissions(
      &test_file,
      std::fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    assert!(
      !has_insecure_permissions(&test_file).unwrap(),
      "File with 400 should be secure (owner read-only)"
    );

    // Cleanup
    std::fs::remove_file(test_file).ok();
  }
}
