//! Reconstruct historical database state from a snapshot for export.
//!
//! Two formats are supported: `json` (a map of qualified table name → array of
//! row objects) and `sql` (`INSERT` statements that recreate the rows).

use anyhow::Result;
use dbsnap_core::TableSnapshot;
use serde_json::Value;

/// Output formats for [`export`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Sql,
}

/// Export the given snapshots, optionally restricted to a single qualified
/// table name (`schema.table`).
pub fn export(snaps: &[TableSnapshot], format: Format, table: Option<&str>) -> Result<String> {
    let selected: Vec<&TableSnapshot> = snaps
        .iter()
        .filter(|s| table.is_none_or(|t| s.schema.qualified() == t))
        .collect();

    match format {
        Format::Json => export_json(&selected),
        Format::Sql => Ok(export_sql(&selected)),
    }
}

fn export_json(snaps: &[&TableSnapshot]) -> Result<String> {
    let mut map = serde_json::Map::new();
    for s in snaps {
        let rows: Vec<Value> = s.rows.iter().map(|r| r.data.clone()).collect();
        map.insert(s.schema.qualified(), Value::Array(rows));
    }
    Ok(serde_json::to_string_pretty(&Value::Object(map))?)
}

fn export_sql(snaps: &[&TableSnapshot]) -> String {
    let mut out = String::new();
    for s in snaps {
        let cols: Vec<&str> = s.schema.columns.iter().map(|c| c.name.as_str()).collect();
        let col_list = cols
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "-- {} ({} rows)\n",
            s.schema.qualified(),
            s.rows.len()
        ));

        for row in &s.rows {
            let values: Vec<String> = cols
                .iter()
                .map(|c| sql_literal(row.data.get(*c).unwrap_or(&Value::Null)))
                .collect();
            out.push_str(&format!(
                "INSERT INTO {}.{} ({}) VALUES ({});\n",
                quote_ident(&s.schema.schema),
                quote_ident(&s.schema.name),
                col_list,
                values.join(", "),
            ));
        }
        out.push('\n');
    }
    out
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Render a JSON value as a PostgreSQL literal.
fn sql_literal(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => {
            if *b {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => quote_str(s),
        // Arrays/objects come from json/jsonb columns: emit as a jsonb literal.
        other => format!("{}::jsonb", quote_str(&other.to_string())),
    }
}

fn quote_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
