//! Engine registry — the composition root that maps a connection string to a
//! concrete [`SnapshotSource`].
//!
//! `dbsnap-engine` and everything below it are engine-agnostic (they only see
//! the [`SnapshotSource`] trait). This module is the *one* place that knows the
//! set of concrete database engines, selected at runtime by the URL scheme.
//!
//! ## Adding a new engine (e.g. MySQL)
//! 1. Add a `dbsnap-mysql` crate implementing [`SnapshotSource`] for `MySqlSource`.
//! 2. Add it as a dependency of `dbsnap-cli`.
//! 3. Add an [`Engine`] variant + scheme mapping in [`Engine::from_url`].
//! 4. Add an [`AnySource`] variant + arm in `connect` and the trait impl.
//!
//! No other crate changes — the engine/diff/integrity/storage layers are
//! already generic over the source.

use anyhow::{bail, Result};
use dbsnap_core::{SnapshotSource, TableSnapshot};
use dbsnap_mysql::MySqlSource;
use dbsnap_postgres::PgSource;

/// The database engines dbsnap knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Postgres,
    MySql,
}

impl Engine {
    /// Determine the engine from a connection string's URL scheme.
    ///
    /// Recognises `postgres://` / `postgresql://` and `mysql://` / `mariadb://`.
    /// Engines that are planned but not yet implemented produce a clear,
    /// actionable error rather than a confusing connection failure.
    pub fn from_url(url: &str) -> Result<Engine> {
        let scheme = url
            .split("://")
            .next()
            .unwrap_or("")
            .split(':') // handle e.g. `sqlite::memory:`
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        match scheme.as_str() {
            "postgres" | "postgresql" => Ok(Engine::Postgres),
            "mysql" | "mariadb" => Ok(Engine::MySql),
            "sqlite" | "file" => {
                bail!("SQLite support is planned but not implemented yet (supported: postgres, mysql)")
            }
            "" => bail!("could not determine the database engine from the connection string"),
            other => {
                bail!("unsupported database engine '{other}://' (supported: postgres, mysql)")
            }
        }
    }
}

/// A concrete snapshot source for any supported engine.
///
/// Uses enum dispatch (not `dyn`) because [`SnapshotSource::capture`] is an
/// `async fn`: the engine layer stays generic over `S: SnapshotSource`, and
/// this enum is simply one such `S` that fans out to the chosen backend.
pub enum AnySource {
    Postgres(PgSource),
    MySql(MySqlSource),
}

impl AnySource {
    /// Connect to the database named by `url`, selecting the engine by scheme.
    pub async fn connect(url: &str) -> Result<Self> {
        match Engine::from_url(url)? {
            Engine::Postgres => Ok(AnySource::Postgres(PgSource::connect(url).await?)),
            Engine::MySql => Ok(AnySource::MySql(MySqlSource::connect(url).await?)),
        }
    }
}

impl SnapshotSource for AnySource {
    async fn capture(&self, schema: &str) -> Result<Vec<TableSnapshot>> {
        match self {
            AnySource::Postgres(s) => s.capture(schema).await,
            AnySource::MySql(s) => s.capture(schema).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;

    #[test]
    fn detects_postgres_schemes() {
        assert_eq!(
            Engine::from_url("postgres://u:p@h/db").unwrap(),
            Engine::Postgres
        );
        assert_eq!(
            Engine::from_url("postgresql://h/db").unwrap(),
            Engine::Postgres
        );
    }

    #[test]
    fn detects_mysql_schemes() {
        assert_eq!(Engine::from_url("mysql://u:p@h/db").unwrap(), Engine::MySql);
        assert_eq!(Engine::from_url("mariadb://h/db").unwrap(), Engine::MySql);
    }

    #[test]
    fn planned_engines_error_clearly() {
        let err = Engine::from_url("sqlite://./x.db").unwrap_err().to_string();
        assert!(err.contains("SQLite"), "got: {err}");
    }

    #[test]
    fn unknown_and_empty_schemes_error() {
        assert!(Engine::from_url("mongodb://h/db").is_err());
        assert!(Engine::from_url("not-a-url").is_err());
    }
}
