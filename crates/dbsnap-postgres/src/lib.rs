//! PostgreSQL snapshot source.
//!
//! Every row is captured through `to_jsonb(t)`, giving one generic, type-aware
//! path for all column types instead of decoding each Postgres type by hand.
//! Rows are read in primary-key order (or canonical-JSON order when no PK
//! exists) so capture is reproducible.

use anyhow::{Context, Result};
use dbsnap_core::{make_record, Column, SnapshotSource, TableSchema, TableSnapshot};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::collections::HashSet;

#[cfg(test)]
mod tests;

/// A live connection pool to a PostgreSQL database.
pub struct PgSource {
    pool: PgPool,
}

impl PgSource {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .context("connecting to PostgreSQL")?;
        Ok(Self { pool })
    }

    /// List base tables in the given schema, ordered by name.
    pub async fn list_tables(&self, schema: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = $1 AND table_type = 'BASE TABLE' \
             ORDER BY table_name",
        )
        .bind(schema)
        .fetch_all(&self.pool)
        .await
        .context("listing tables")?;

        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("table_name"))
            .collect())
    }

    /// Introspect a table's columns and primary key.
    pub async fn table_schema(&self, schema: &str, table: &str) -> Result<TableSchema> {
        let cols = sqlx::query(
            "SELECT column_name, data_type, is_nullable, ordinal_position \
             FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2 \
             ORDER BY ordinal_position",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("introspecting columns of {schema}.{table}"))?;

        let pk_rows = sqlx::query(
            "SELECT a.attname AS column_name \
             FROM pg_index i \
             JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
             WHERE i.indrelid = format('%I.%I', $1, $2)::regclass AND i.indisprimary \
             ORDER BY array_position(i.indkey, a.attnum)",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("introspecting primary key of {schema}.{table}"))?;

        let primary_key: Vec<String> = pk_rows
            .iter()
            .map(|r| r.get::<String, _>("column_name"))
            .collect();
        let pk_set: HashSet<&str> = primary_key.iter().map(|s| s.as_str()).collect();

        let columns = cols
            .iter()
            .map(|r| {
                let name: String = r.get("column_name");
                let is_primary_key = pk_set.contains(name.as_str());
                Column {
                    data_type: r.get("data_type"),
                    nullable: r.get::<String, _>("is_nullable") == "YES",
                    ordinal: r.get::<i32, _>("ordinal_position"),
                    is_primary_key,
                    name,
                }
            })
            .collect();

        Ok(TableSchema {
            schema: schema.to_string(),
            name: table.to_string(),
            columns,
            primary_key,
        })
    }

    /// Capture every row of one table as a [`TableSnapshot`].
    pub async fn snapshot_table(&self, schema: &TableSchema) -> Result<TableSnapshot> {
        let order_by = if schema.primary_key.is_empty() {
            // Deterministic even without a PK: order by the canonical row text.
            "(to_jsonb(t))::text".to_string()
        } else {
            schema
                .primary_key
                .iter()
                .map(|c| format!("t.{}", quote_ident(c)))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let sql = format!(
            "SELECT to_jsonb(t) AS row FROM {}.{} t ORDER BY {}",
            quote_ident(&schema.schema),
            quote_ident(&schema.name),
            order_by,
        );

        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .with_context(|| format!("snapshotting {}", schema.qualified()))?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            let data: serde_json::Value = row.try_get("row")?;
            records.push(make_record(schema, data));
        }

        tracing::debug!(table = %schema.qualified(), rows = records.len(), "captured table");
        Ok(TableSnapshot {
            schema: schema.clone(),
            rows: records,
        })
    }

    /// Capture the entire schema: introspect and snapshot every base table.
    pub async fn snapshot_all(&self, schema: &str) -> Result<Vec<TableSnapshot>> {
        let mut snapshots = Vec::new();
        for table in self.list_tables(schema).await? {
            let ts = self.table_schema(schema, &table).await?;
            snapshots.push(self.snapshot_table(&ts).await?);
        }
        Ok(snapshots)
    }
}

/// The blanket database abstraction: a Postgres connection is one concrete
/// [`SnapshotSource`]. The engine depends only on this trait.
impl SnapshotSource for PgSource {
    async fn capture(&self, schema: &str) -> Result<Vec<TableSnapshot>> {
        self.snapshot_all(schema).await
    }
}

/// Quote a SQL identifier, doubling embedded quotes. Identifiers come from the
/// database catalog (not user input), but we quote anyway for correctness with
/// mixed-case / reserved names.
pub(crate) fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}
