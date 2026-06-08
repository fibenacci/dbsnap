//! Integrity verification of the stored history.
//!
//! Verification recomputes every hash from stored content and checks it against
//! the recorded value, then checks the parent links form an unbroken chain.
//! Any mismatch means an object was altered out of band (the `.dbsnap/` store
//! was tampered with, or a write was corrupted).

use anyhow::Result;
use dbsnap_hashing::hash_json;
use dbsnap_storage::Store;
use serde::Serialize;

/// A single detected integrity problem.
#[derive(Debug, Clone, Serialize)]
pub struct Violation {
    pub kind: String,
    pub detail: String,
}

impl Violation {
    fn new(kind: &str, detail: impl Into<String>) -> Self {
        Violation {
            kind: kind.to_string(),
            detail: detail.into(),
        }
    }
}

/// Outcome of a verification run.
#[derive(Debug, Clone, Serialize, Default)]
pub struct VerifyReport {
    pub commits_checked: usize,
    pub tables_checked: usize,
    pub rows_checked: u64,
    pub violations: Vec<Violation>,
}

impl VerifyReport {
    pub fn ok(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Verify the full commit chain reachable from HEAD.
pub fn verify_chain(store: &Store) -> Result<VerifyReport> {
    let mut report = VerifyReport::default();

    let head = match store.head()? {
        Some(h) => h,
        None => return Ok(report), // empty repo: trivially intact
    };

    let chain = store.chain(&head)?;
    let mut expected_parent: Option<dbsnap_hashing::DbHash> = None;

    for (stored_hash, commit) in &chain {
        report.commits_checked += 1;

        // 1. Commit hash must match the content (and thus its filename/ref).
        let recomputed = commit.hash();
        if recomputed != *stored_hash {
            report.violations.push(Violation::new(
                "commit-hash-mismatch",
                format!(
                    "commit {} content hashes to {}",
                    stored_hash.short(),
                    recomputed.short()
                ),
            ));
        }

        // 2. Parent linkage: each commit must be the parent the child claimed.
        if let Some(parent) = expected_parent {
            if *stored_hash != parent {
                report.violations.push(Violation::new(
                    "broken-chain",
                    format!(
                        "expected parent {} but found {}",
                        parent.short(),
                        stored_hash.short()
                    ),
                ));
            }
        }
        expected_parent = commit.parent;

        // 3. Tree must rehash to the value the commit recorded.
        let tree = store.read_tree(&commit.tree)?;
        if tree.hash() != commit.tree {
            report.violations.push(Violation::new(
                "tree-hash-mismatch",
                format!(
                    "tree {} in commit {} is altered",
                    commit.tree.short(),
                    stored_hash.short()
                ),
            ));
        }

        // 4. Each table object: schema hash, table hash, and every row hash.
        for entry in &tree.entries {
            report.tables_checked += 1;
            let snap = store.read_table_snapshot(&entry.table_hash)?;

            if snap.schema.schema_hash() != entry.schema_hash {
                report.violations.push(Violation::new(
                    "schema-hash-mismatch",
                    format!("table {} schema altered", entry.table),
                ));
            }
            if snap.table_hash() != entry.table_hash {
                report.violations.push(Violation::new(
                    "table-hash-mismatch",
                    format!("table {} data altered", entry.table),
                ));
            }
            if snap.rows.len() as u64 != entry.row_count {
                report.violations.push(Violation::new(
                    "row-count-mismatch",
                    format!(
                        "table {} expected {} rows, stored {}",
                        entry.table,
                        entry.row_count,
                        snap.rows.len()
                    ),
                ));
            }

            for row in &snap.rows {
                report.rows_checked += 1;
                if hash_json(&row.data) != row.hash {
                    report.violations.push(Violation::new(
                        "row-hash-mismatch",
                        format!(
                            "table {} primary key {} hashes to {} (expected {})",
                            entry.table,
                            row.pk,
                            hash_json(&row.data).short(),
                            row.hash.short(),
                        ),
                    ));
                }
            }
        }
    }

    Ok(report)
}
