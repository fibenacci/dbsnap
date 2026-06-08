use std::path::PathBuf;

use anyhow::{bail, Result};
use dbsnap_engine::ExportFormat;

use crate::context;

pub fn run(
    commit: String,
    format: String,
    table: Option<String>,
    output: Option<PathBuf>,
) -> Result<()> {
    let repo = context::open()?;
    let fmt = match format.as_str() {
        "json" => ExportFormat::Json,
        "sql" => ExportFormat::Sql,
        other => bail!("unknown format '{other}'"),
    };

    let out = repo.export(&commit, fmt, table.as_deref())?;
    match output {
        Some(path) => {
            std::fs::write(&path, out)?;
            println!("Exported to {}", path.display());
        }
        None => print!("{out}"),
    }
    Ok(())
}
