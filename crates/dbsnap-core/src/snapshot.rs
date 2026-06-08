//! Captured table state: rows with their identity and content hash.

use dbsnap_hashing::{hash_json, DbHash, Hasher};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::schema::TableSchema;

/// One captured row: its primary-key identity, its content hash, and the
/// full canonical JSON value (kept so we can diff and export later).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowRecord {
    /// Stable identity of the row within its table (PK columns joined, or the
    /// whole row's canonical JSON if the table has no primary key).
    pub pk: String,
    pub hash: DbHash,
    pub data: Value,
}

/// A full snapshot of one table: schema plus every row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSnapshot {
    pub schema: TableSchema,
    pub rows: Vec<RowRecord>,
}

impl TableSnapshot {
    /// Deterministic hash of the table's full state.
    ///
    /// Rows are sorted by primary key first, so the hash is independent of the
    /// order in which rows were read from the database.
    pub fn table_hash(&self) -> DbHash {
        let mut sorted: Vec<&RowRecord> = self.rows.iter().collect();
        sorted.sort_by(|a, b| a.pk.cmp(&b.pk));

        let mut h = Hasher::new("table");
        h.update_hash(&self.schema.schema_hash());
        for r in sorted {
            h.update_str(&r.pk);
            h.update_hash(&r.hash);
        }
        h.finalize()
    }
}

/// Compute the primary-key identity string for a row's JSON value given its
/// schema. Shared by the capture path and any consistency checks.
pub fn row_pk(schema: &TableSchema, data: &Value) -> String {
    if schema.primary_key.is_empty() {
        // No PK: identity is the whole canonical row.
        serde_json::to_string(data).unwrap_or_default()
    } else {
        schema
            .primary_key
            .iter()
            .map(|c| {
                data.get(c)
                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("\u{1f}")
    }
}

/// Build a [`RowRecord`] from a raw row value and its schema.
pub fn make_record(schema: &TableSchema, data: Value) -> RowRecord {
    let pk = row_pk(schema, &data);
    let hash = hash_json(&data);
    RowRecord { pk, hash, data }
}
