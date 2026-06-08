//! Table structure: columns and schemas, plus their deterministic hash.

use dbsnap_hashing::{DbHash, Hasher};
use serde::{Deserialize, Serialize};

/// A single column definition, as introspected from the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub ordinal: i32,
    pub is_primary_key: bool,
}

/// Schema of a single table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableSchema {
    pub schema: String,
    pub name: String,
    pub columns: Vec<Column>,
    /// Primary-key column names, in key order. Empty if the table has no PK.
    pub primary_key: Vec<String>,
}

impl TableSchema {
    /// Fully qualified `schema.table` identifier used as the table's key.
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    /// Deterministic hash of the structural definition (not the data).
    pub fn schema_hash(&self) -> DbHash {
        let mut h = Hasher::new("schema");
        h.update_str(&self.schema);
        h.update_str(&self.name);
        for c in &self.columns {
            h.update_str(&c.name);
            h.update_str(&c.data_type);
            h.update(&[c.nullable as u8, c.is_primary_key as u8]);
            h.update(&c.ordinal.to_le_bytes());
        }
        for pk in &self.primary_key {
            h.update_str(pk);
        }
        h.finalize()
    }
}
