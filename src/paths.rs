//! Terminai-owned configuration and cache paths.

use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn config_dir() -> Result<PathBuf> {
  #[cfg(windows)]
  {
    return std::env::var_os("APPDATA")
      .map(PathBuf::from)
      .map(|path| path.join("terminai"))
      .context(
        "APPDATA is not set; cannot determine Terminai config directory",
      );
  }
  #[cfg(not(windows))]
  {
    let root = std::env::var_os("XDG_CONFIG_HOME")
      .map(PathBuf::from)
      .or_else(|| {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
      })
      .context("cannot determine Terminai config directory")?;
    Ok(root.join("terminai"))
  }
}

pub fn cache_dir() -> Result<PathBuf> {
  #[cfg(windows)]
  {
    return std::env::var_os("LOCALAPPDATA")
      .map(PathBuf::from)
      .map(|path| path.join("terminai"))
      .context(
        "LOCALAPPDATA is not set; cannot determine Terminai cache directory",
      );
  }
  #[cfg(not(windows))]
  {
    let root = std::env::var_os("XDG_CACHE_HOME")
      .map(PathBuf::from)
      .or_else(|| {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache"))
      })
      .context("cannot determine Terminai cache directory")?;
    Ok(root.join("terminai"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn directories_end_in_terminai() {
    assert!(config_dir().unwrap().ends_with("terminai"));
    assert!(cache_dir().unwrap().ends_with("terminai"));
  }
}
