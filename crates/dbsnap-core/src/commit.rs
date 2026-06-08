//! The Merkle tree of a captured database and the commit objects that chain it.

use dbsnap_hashing::{DbHash, Hasher};
use serde::{Deserialize, Serialize};

use crate::snapshot::TableSnapshot;

/// One table's entry in a snapshot tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeEntry {
    pub table: String,
    pub table_hash: DbHash,
    pub schema_hash: DbHash,
    pub row_count: u64,
}

/// The set of all tables captured in one commit — the database "tree".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    /// Build a tree from captured table snapshots (entries sorted by name).
    pub fn from_snapshots(snaps: &[TableSnapshot]) -> Tree {
        let mut entries: Vec<TreeEntry> = snaps
            .iter()
            .map(|s| TreeEntry {
                table: s.schema.qualified(),
                table_hash: s.table_hash(),
                schema_hash: s.schema.schema_hash(),
                row_count: s.rows.len() as u64,
            })
            .collect();
        entries.sort_by(|a, b| a.table.cmp(&b.table));
        Tree { entries }
    }

    /// Deterministic hash of the whole database state.
    pub fn hash(&self) -> DbHash {
        let mut entries = self.entries.clone();
        entries.sort_by(|a, b| a.table.cmp(&b.table));

        let mut h = Hasher::new("tree");
        for e in &entries {
            h.update_str(&e.table);
            h.update_hash(&e.table_hash);
            h.update_hash(&e.schema_hash);
            h.update(&e.row_count.to_le_bytes());
        }
        h.finalize()
    }

    pub fn entry(&self, table: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.table == table)
    }

    /// Total rows across all tables in this tree.
    pub fn total_rows(&self) -> u64 {
        self.entries.iter().map(|e| e.row_count).sum()
    }
}

/// An immutable, append-only commit linking a tree to its parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub tree: DbHash,
    pub parent: Option<DbHash>,
    pub message: String,
    /// Unix epoch seconds. Excluded from *state* hashes but part of the commit
    /// identity, exactly like a Git commit's author date.
    pub timestamp: i64,
    pub author: String,
}

impl Commit {
    /// The commit's identity hash. Folding in `parent` makes the chain
    /// tamper-evident: altering any ancestor changes every descendant hash.
    pub fn hash(&self) -> DbHash {
        let mut h = Hasher::new("commit");
        h.update_hash(&self.tree);
        match &self.parent {
            Some(p) => {
                h.update(b"P");
                h.update_hash(p);
            }
            None => {
                h.update(b"R"); // root commit
            }
        }
        h.update_str(&self.message);
        h.update(&self.timestamp.to_le_bytes());
        h.update_str(&self.author);
        h.finalize()
    }

    /// First line of the commit message, for one-line summaries.
    pub fn summary(&self) -> &str {
        self.message.lines().next().unwrap_or("")
    }
}
