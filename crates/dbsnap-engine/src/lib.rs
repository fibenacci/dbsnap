//! High-level orchestration: the [`Repository`] ties the storage backend, the
//! snapshot source, and the diff / integrity / export engines into the handful
//! of domain operations the CLI (or any other front-end) actually performs.
//!
//! The split of responsibilities is deliberate:
//! - `dbsnap-storage` is *persistence only* (read/write objects, refs).
//! - `dbsnap-engine` is *workflow* (capture → store → commit, verify, diff…).
//! - the CLI is *presentation* (arg parsing, rendering, exit codes).
//!
//! Every operation that touches a live database is generic over
//! [`SnapshotSource`], so the engine never names PostgreSQL and is fully
//! testable with an in-memory fake (see `tests/commit.rs`).

use std::path::Path;

use anyhow::{Context, Result};
use dbsnap_core::{now_unix, Commit, DbHash, SnapshotSource, TableSnapshot, Tree};
use dbsnap_diff::{diff_snapshots, SnapshotDiff};
use dbsnap_export::{export, Format};
use dbsnap_integrity::{verify_chain, VerifyReport};
use dbsnap_storage::{Config, Store};

// Re-export the types front-ends need so they can depend on the engine alone.
pub use dbsnap_export::Format as ExportFormat;
pub use dbsnap_storage::Config as RepoConfig;

/// Result of a [`Repository::commit`] call.
#[derive(Debug, Clone)]
pub enum CommitOutcome {
    /// A new commit was recorded.
    Created {
        commit: DbHash,
        tree: DbHash,
        tables: usize,
        rows: u64,
    },
    /// Nothing changed since `head`; no commit was written.
    Unchanged { head: DbHash },
}

/// Summary of repository state for `status`.
#[derive(Debug, Clone)]
pub struct Status {
    /// `None` when no commits exist yet.
    pub head: Option<(DbHash, Commit)>,
    pub tables: usize,
    pub rows: u64,
}

/// A dbsnap repository: the entry point for all high-level operations.
pub struct Repository {
    store: Store,
}

impl Repository {
    // ----- lifecycle ------------------------------------------------------

    /// Create a new repository under `parent/.dbsnap`.
    pub fn init(parent: &Path, config: Config) -> Result<Self> {
        Ok(Self {
            store: Store::init(parent, config)?,
        })
    }

    /// Open the repository containing `start` (walks up to find `.dbsnap`).
    pub fn discover(start: &Path) -> Result<Self> {
        Ok(Self {
            store: Store::discover(start)?,
        })
    }

    /// The persistent configuration (schema, optional connection string).
    pub fn config(&self) -> &Config {
        &self.store.config
    }

    /// Filesystem path of the `.dbsnap` directory.
    pub fn path(&self) -> &Path {
        &self.store.root
    }

    // ----- commit ---------------------------------------------------------

    /// Capture the current state of `source` and record it as a new commit.
    /// Returns [`CommitOutcome::Unchanged`] if state matches the current HEAD.
    pub async fn commit<S: SnapshotSource>(
        &self,
        source: &S,
        message: String,
        author: String,
    ) -> Result<CommitOutcome> {
        let snapshots = source.capture(&self.store.config.schema).await?;

        for snap in &snapshots {
            self.store.write_table_snapshot(snap)?;
        }
        let tree = Tree::from_snapshots(&snapshots);
        let tree_hash = self.store.write_tree(&tree)?;

        let parent = self.store.head()?;
        if let Some(parent_hash) = parent {
            if self.store.read_commit(&parent_hash)?.tree == tree_hash {
                return Ok(CommitOutcome::Unchanged { head: parent_hash });
            }
        }

        let commit = Commit {
            tree: tree_hash,
            parent,
            message,
            timestamp: now_unix(),
            author,
        };
        let hash = self.store.write_commit(&commit)?;
        self.store.set_head(&hash)?;

        Ok(CommitOutcome::Created {
            commit: hash,
            tree: tree_hash,
            tables: tree.entries.len(),
            rows: tree.total_rows(),
        })
    }

    // ----- history & inspection ------------------------------------------

    /// Commit history from HEAD back to the root (newest first), capped at
    /// `limit` entries when given.
    pub fn history(&self, limit: Option<usize>) -> Result<Vec<(DbHash, Commit)>> {
        let head = match self.store.head()? {
            Some(h) => h,
            None => return Ok(Vec::new()),
        };
        let mut chain = self.store.chain(&head)?;
        if let Some(n) = limit {
            chain.truncate(n);
        }
        Ok(chain)
    }

    pub fn status(&self) -> Result<Status> {
        match self.store.head()? {
            None => Ok(Status {
                head: None,
                tables: 0,
                rows: 0,
            }),
            Some(hash) => {
                let commit = self.store.read_commit(&hash)?;
                let tree = self.store.read_tree(&commit.tree)?;
                Ok(Status {
                    tables: tree.entries.len(),
                    rows: tree.total_rows(),
                    head: Some((hash, commit)),
                })
            }
        }
    }

    /// Load every table snapshot recorded at a reference (`HEAD`, `HEAD~N`,
    /// or a (prefix) hash).
    pub fn snapshots_at(&self, reference: &str) -> Result<Vec<TableSnapshot>> {
        let hash = self.store.resolve(reference)?;
        self.snapshots_of_commit(&hash)
    }

    fn snapshots_of_commit(&self, commit: &DbHash) -> Result<Vec<TableSnapshot>> {
        let commit = self.store.read_commit(commit)?;
        let tree = self.store.read_tree(&commit.tree)?;
        tree.entries
            .iter()
            .map(|e| self.store.read_table_snapshot(&e.table_hash))
            .collect()
    }

    // ----- diff -----------------------------------------------------------

    /// Semantic diff between two stored references.
    pub fn diff(&self, old: &str, new: &str) -> Result<SnapshotDiff> {
        let old_snaps = self.snapshots_at(old)?;
        let new_snaps = self.snapshots_at(new)?;
        Ok(diff_snapshots(&old_snaps, &new_snaps))
    }

    /// Diff the live database against HEAD — i.e. uncommitted / out-of-band
    /// changes. Errors if there is no HEAD to compare against.
    pub async fn live_diff<S: SnapshotSource>(&self, source: &S) -> Result<SnapshotDiff> {
        let head = self
            .store
            .head()?
            .context("no HEAD to compare the database against")?;
        let recorded = self.snapshots_of_commit(&head)?;
        let live = source.capture(&self.store.config.schema).await?;
        Ok(diff_snapshots(&recorded, &live))
    }

    // ----- verify & export -----------------------------------------------

    /// Recompute and verify the stored hash chain.
    pub fn verify(&self) -> Result<VerifyReport> {
        verify_chain(&self.store)
    }

    /// Export the state recorded at a reference, optionally one table only.
    pub fn export(&self, reference: &str, format: Format, table: Option<&str>) -> Result<String> {
        let snaps = self.snapshots_at(reference)?;
        export(&snaps, format, table)
    }
}
