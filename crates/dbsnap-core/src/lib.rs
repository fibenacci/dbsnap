//! Core domain model and the deterministic hash hierarchy.
//!
//! The hashing forms a Merkle structure, Git-style:
//!
//! ```text
//!   row hash      = H(canonical_json(row))
//!   table hash    = H(schema_hash, [ (pk, row_hash) sorted by pk ])
//!   tree hash     = H([ (table, table_hash, schema_hash, row_count) sorted ])
//!   commit hash   = H(tree, parent, message, timestamp, author)
//! ```
//!
//! Row/table/tree hashes depend only on database *state*, so identical state
//! always yields identical hashes (the determinism guarantee). The commit hash
//! additionally folds in the parent commit, forming a tamper-evident chain.
//!
//! ## Module map
//! - `schema`   — column / table structure ([`Column`], [`TableSchema`])
//! - `snapshot` — captured rows ([`RowRecord`], [`TableSnapshot`])
//! - `commit`   — the Merkle tree and commit objects ([`Tree`], [`Commit`])
//! - `source`   — the [`SnapshotSource`] abstraction over a database

mod commit;
mod schema;
mod snapshot;
mod source;

pub use commit::{Commit, Tree, TreeEntry};
pub use schema::{Column, TableSchema};
pub use snapshot::{make_record, row_pk, RowRecord, TableSnapshot};
pub use source::SnapshotSource;

/// Re-exported so downstream crates can name hashes via `dbsnap_core::DbHash`.
pub use dbsnap_hashing::DbHash;
