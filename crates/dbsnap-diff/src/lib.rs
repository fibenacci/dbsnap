//! Semantic diffing between two snapshots.
//!
//! Comparison is by primary key and row hash: rows present only in the new
//! snapshot are inserts, only in the old are deletes, and rows whose hash
//! changed are updates. For updates we descend into the JSON to report the
//! exact columns that changed (old value → new value).

use dbsnap_core::TableSnapshot;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// A single column's change within an updated row.
#[derive(Debug, Clone, Serialize)]
pub struct ColumnChange {
    pub column: String,
    pub old: Value,
    pub new: Value,
}

/// An updated row and the columns that changed.
#[derive(Debug, Clone, Serialize)]
pub struct RowChange {
    pub pk: String,
    pub columns: Vec<ColumnChange>,
}

/// Per-table change summary.
#[derive(Debug, Clone, Serialize, Default)]
pub struct TableDiff {
    pub table: String,
    pub inserted: Vec<String>,
    pub deleted: Vec<String>,
    pub updated: Vec<RowChange>,
}

impl TableDiff {
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty() && self.deleted.is_empty() && self.updated.is_empty()
    }
}

/// Diff across an entire database tree.
#[derive(Debug, Clone, Serialize, Default)]
pub struct SnapshotDiff {
    pub added_tables: Vec<String>,
    pub removed_tables: Vec<String>,
    /// Per-table diffs for tables present in both snapshots (non-empty ones).
    pub tables: Vec<TableDiff>,
}

impl SnapshotDiff {
    pub fn is_empty(&self) -> bool {
        self.added_tables.is_empty() && self.removed_tables.is_empty() && self.tables.is_empty()
    }
}

/// Diff two snapshots of the *same* table.
pub fn diff_tables(old: &TableSnapshot, new: &TableSnapshot) -> TableDiff {
    let table = new.schema.qualified();
    let old_rows: BTreeMap<&str, &dbsnap_core::RowRecord> =
        old.rows.iter().map(|r| (r.pk.as_str(), r)).collect();
    let new_rows: BTreeMap<&str, &dbsnap_core::RowRecord> =
        new.rows.iter().map(|r| (r.pk.as_str(), r)).collect();

    let mut diff = TableDiff {
        table,
        ..Default::default()
    };

    for (pk, nr) in &new_rows {
        match old_rows.get(pk) {
            None => diff.inserted.push((*pk).to_string()),
            Some(or) if or.hash != nr.hash => {
                diff.updated.push(RowChange {
                    pk: (*pk).to_string(),
                    columns: column_changes(&or.data, &nr.data),
                });
            }
            Some(_) => {} // unchanged
        }
    }
    for pk in old_rows.keys() {
        if !new_rows.contains_key(pk) {
            diff.deleted.push((*pk).to_string());
        }
    }

    diff.inserted.sort();
    diff.deleted.sort();
    diff.updated.sort_by(|a, b| a.pk.cmp(&b.pk));
    diff
}

/// Compute the per-column changes between two row JSON objects.
fn column_changes(old: &Value, new: &Value) -> Vec<ColumnChange> {
    let mut changes = Vec::new();
    let mut keys: Vec<&String> = Vec::new();
    if let Some(m) = old.as_object() {
        keys.extend(m.keys());
    }
    if let Some(m) = new.as_object() {
        for k in m.keys() {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    keys.sort();

    for k in keys {
        let ov = old.get(k).cloned().unwrap_or(Value::Null);
        let nv = new.get(k).cloned().unwrap_or(Value::Null);
        if ov != nv {
            changes.push(ColumnChange {
                column: k.clone(),
                old: ov,
                new: nv,
            });
        }
    }
    changes
}

/// Diff two full database snapshots (lists of table snapshots).
pub fn diff_snapshots(old: &[TableSnapshot], new: &[TableSnapshot]) -> SnapshotDiff {
    let old_by: BTreeMap<String, &TableSnapshot> =
        old.iter().map(|t| (t.schema.qualified(), t)).collect();
    let new_by: BTreeMap<String, &TableSnapshot> =
        new.iter().map(|t| (t.schema.qualified(), t)).collect();

    let mut result = SnapshotDiff::default();

    for (name, nt) in &new_by {
        match old_by.get(name) {
            None => result.added_tables.push(name.clone()),
            Some(ot) => {
                let d = diff_tables(ot, nt);
                if !d.is_empty() {
                    result.tables.push(d);
                }
            }
        }
    }
    for name in old_by.keys() {
        if !new_by.contains_key(name) {
            result.removed_tables.push(name.clone());
        }
    }

    result.added_tables.sort();
    result.removed_tables.sort();
    result.tables.sort_by(|a, b| a.table.cmp(&b.table));
    result
}
