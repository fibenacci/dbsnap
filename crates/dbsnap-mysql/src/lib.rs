//! MySQL / MariaDB snapshot source.
//!
//! Mirrors `dbsnap-postgres`: every row is captured as canonical JSON so all
//! column types share one generic path. Postgres has `to_jsonb(t)`; MySQL has
//! no whole-row equivalent, so we build a `JSON_OBJECT(...)` from the
//! introspected column list. Rows are read in primary-key order (or by all
//! columns when there is no PK) so capture is reproducible.
//!
//! Determinism is per-engine: MySQL renders types differently from Postgres,
//! so a MySQL snapshot hash is only comparable with other MySQL snapshots.

use anyhow::{Context, Result};
use dbsnap_core::{make_record, Column, SnapshotSource, TableSchema, TableSnapshot};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{MySqlPool, Row};
use std::collections::HashSet;

#[cfg(test)]
mod tests;

/// A live connection pool to a MySQL / MariaDB database.
pub struct MySqlSource {
    pool: MySqlPool,
}

impl MySqlSource {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .context("connecting to MySQL")?;
        Ok(Self { pool })
    }

    /// List base tables in the given database (MySQL's "schema"), ordered by name.
    async fn list_tables(&self, schema: &str) -> Result<Vec<String>> {
        // information_schema string columns come back as VARBINARY under some
        // MySQL collations; CAST(... AS CHAR) makes them decode as text.
        let rows = sqlx::query(
            "SELECT CAST(TABLE_NAME AS CHAR) AS table_name FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = ? AND TABLE_TYPE = 'BASE TABLE' \
             ORDER BY TABLE_NAME",
        )
        .bind(schema)
        .fetch_all(&self.pool)
        .await
        .context("listing tables")?;

        let mut tables = Vec::with_capacity(rows.len());
        for r in &rows {
            tables.push(r.try_get::<String, _>("table_name")?);
        }
        Ok(tables)
    }

    /// Introspect a table's columns and primary key.
    async fn table_schema(&self, schema: &str, table: &str) -> Result<TableSchema> {
        let cols = sqlx::query(
            "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name, CAST(DATA_TYPE AS CHAR) AS data_type, \
                    CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(ORDINAL_POSITION AS SIGNED) AS ordinal_position \
             FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
             ORDER BY ORDINAL_POSITION",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("introspecting columns of {schema}.{table}"))?;

        let pk_rows = sqlx::query(
            "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name \
             FROM information_schema.KEY_COLUMN_USAGE \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY' \
             ORDER BY ORDINAL_POSITION",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("introspecting primary key of {schema}.{table}"))?;

        let mut primary_key = Vec::with_capacity(pk_rows.len());
        for r in &pk_rows {
            primary_key.push(r.try_get::<String, _>("column_name")?);
        }
        let pk_set: HashSet<&str> = primary_key.iter().map(|s| s.as_str()).collect();

        let mut columns = Vec::with_capacity(cols.len());
        for r in &cols {
            let name: String = r.try_get("column_name")?;
            let is_primary_key = pk_set.contains(name.as_str());
            columns.push(Column {
                data_type: r.try_get("data_type")?,
                nullable: r.try_get::<String, _>("is_nullable")? == "YES",
                ordinal: r.try_get::<i64, _>("ordinal_position")? as i32,
                is_primary_key,
                name,
            });
        }

        Ok(TableSchema {
            schema: schema.to_string(),
            name: table.to_string(),
            columns,
            primary_key,
        })
    }

    /// Capture every row of one table as a [`TableSnapshot`].
    async fn snapshot_table(&self, schema: &TableSchema) -> Result<TableSnapshot> {
        // Build `JSON_OBJECT('col', `col`, ...)` over the introspected columns.
        let pairs = schema
            .columns
            .iter()
            .map(|c| format!("{}, {}", sql_str_lit(&c.name), quote_ident(&c.name)))
            .collect::<Vec<_>>()
            .join(", ");

        // Deterministic ordering: by primary key, else by every column.
        let order_cols: Vec<String> = if schema.primary_key.is_empty() {
            schema
                .columns
                .iter()
                .map(|c| quote_ident(&c.name))
                .collect()
        } else {
            schema.primary_key.iter().map(|c| quote_ident(c)).collect()
        };

        let sql = format!(
            "SELECT JSON_OBJECT({pairs}) AS `row` FROM {}.{} ORDER BY {}",
            quote_ident(&schema.schema),
            quote_ident(&schema.name),
            order_cols.join(", "),
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

    /// Capture the entire database: introspect and snapshot every base table.
    async fn snapshot_all(&self, schema: &str) -> Result<Vec<TableSnapshot>> {
        let mut snapshots = Vec::new();
        for table in self.list_tables(schema).await? {
            let ts = self.table_schema(schema, &table).await?;
            snapshots.push(self.snapshot_table(&ts).await?);
        }
        Ok(snapshots)
    }
}

/// A MySQL connection is one concrete [`SnapshotSource`]; the engine layer
/// depends only on this trait.
impl SnapshotSource for MySqlSource {
    async fn capture(&self, schema: &str) -> Result<Vec<TableSnapshot>> {
        self.snapshot_all(schema).await
    }
}

/// Quote a MySQL identifier with backticks, doubling embedded backticks.
pub(crate) fn quote_ident(ident: &str) -> String {
    format!("`{}`", ident.replace('`', "``"))
}

/// Render a string as a MySQL single-quoted literal (used for JSON_OBJECT keys).
/// Escapes both `'` and `\`, which are the two special characters in MySQL
/// string literals under the default `NO_BACKSLASH_ESCAPES`-off mode.
pub(crate) fn sql_str_lit(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "''"))
}
