//! Wiring between the CLI and the engine: locate the repository and construct
//! a concrete database source. This is the only place that names PostgreSQL.

use anyhow::{Context, Result};
use dbsnap_engine::Repository;
use dbsnap_postgres::PgSource;

/// Open the repository containing the current working directory.
pub fn open() -> Result<Repository> {
    Repository::discover(&std::env::current_dir()?)
}

/// Resolve the connection string: the `DATABASE_URL` env var wins over the
/// value stored in `config.toml`, so secrets need not live on disk.
pub fn database_url(repo: &Repository) -> Result<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() {
            return Ok(url);
        }
    }
    repo.config()
        .database_url
        .clone()
        .context("no database connection string; set DATABASE_URL or store one via `dbsnap init`")
}

/// Connect to the configured database as a snapshot source.
pub async fn connect(repo: &Repository) -> Result<PgSource> {
    PgSource::connect(&database_url(repo)?).await
}
