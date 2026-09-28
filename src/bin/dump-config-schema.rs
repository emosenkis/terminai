use std::{fs, path::PathBuf};

use anyhow::Result;
use rmcp::schemars;
use termin::terminai_config::TerminaiConfig;

fn main() -> Result<()> {
  let schema = schemars::schema_for!(TerminaiConfig);
  let json = format!("{}\n", serde_json::to_string_pretty(&schema)?);
  let docs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("..")
    .join("docs");
  for out_path in [
    docs_dir.join(format!("schema-v{}.json", env!("CARGO_PKG_VERSION"))),
    docs_dir.join("schema.json"),
  ] {
    fs::write(&out_path, &json)?;
    println!("{}", out_path.display());
  }

  Ok(())
}
