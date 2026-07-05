use std::{fs, path::PathBuf};

use anyhow::Result;
use rmcp::schemars;
use termin::terminai_config::TerminaiConfig;

fn main() -> Result<()> {
  let schema = schemars::schema_for!(TerminaiConfig);
  let docs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("..")
    .join("docs");
  let out_path =
    docs_dir.join(format!("schema-v{}.json", env!("CARGO_PKG_VERSION")));

  fs::write(&out_path, serde_json::to_string_pretty(&schema)?)?;
  println!("{}", out_path.display());

  Ok(())
}
