//! Abstraction over a snapshottable database.

use anyhow::Result;

use crate::snapshot::TableSnapshot;

/// A database that can be captured into deterministic table snapshots.
///
/// This is dbsnap's main extension point. `dbsnap-postgres::PgSource` is the
/// only implementor today, but MySQL/SQLite sources will implement the same
/// trait, and the engine is generic over it — so commit/diff/verify logic
/// never names a concrete database and can be exercised with an in-memory fake
/// in tests.
///
/// The `async fn in trait` lint is allowed deliberately: all call sites are
/// internal and generic (never `dyn`), so the missing `Send` bound the lint
/// warns about is supplied by the caller's context, not the trait.
#[allow(async_fn_in_trait)]
pub trait SnapshotSource {
    /// Capture every base table in `schema` as deterministic snapshots.
    async fn capture(&self, schema: &str) -> Result<Vec<TableSnapshot>>;
}
