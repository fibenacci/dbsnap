//! Wiring between the CLI and the engine: locate the repository and connect to
//! the configured database. The concrete engine is chosen by the [`crate::source`]
//! registry from the connection string's URL scheme.

use anyhow::{Context, Result};
use dbsnap_engine::Repository;

use crate::source::AnySource;

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

/// Connect to the configured database as a snapshot source, selecting the
/// engine by URL scheme (`postgres://`, and later `mysql://`, `sqlite://`).
pub async fn connect(repo: &Repository) -> Result<AnySource> {
    AnySource::connect(&database_url(repo)?).await
}
