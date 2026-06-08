//! Behavioural tests for export (public API only).

use dbsnap_core::{make_record, Column, TableSchema, TableSnapshot};
use dbsnap_export::{export, Format};
use serde_json::{json, Value};

fn snap() -> TableSnapshot {
    let s = TableSchema {
        schema: "public".into(),
        name: "product".into(),
        columns: vec![
            Column {
                name: "id".into(),
                data_type: "integer".into(),
                nullable: false,
                ordinal: 1,
                is_primary_key: true,
            },
            Column {
                name: "name".into(),
                data_type: "text".into(),
                nullable: true,
                ordinal: 2,
                is_primary_key: false,
            },
        ],
        primary_key: vec!["id".into()],
    };
    TableSnapshot {
        schema: s.clone(),
        rows: vec![make_record(&s, json!({"id": 1, "name": "O'Brien"}))],
    }
}

#[test]
fn sql_escapes_quotes() {
    let out = export(&[snap()], Format::Sql, None).unwrap();
    assert!(out.contains("'O''Brien'"), "got: {out}");
    assert!(out.contains("INSERT INTO \"public\".\"product\""));
}

#[test]
fn json_groups_by_table() {
    let out = export(&[snap()], Format::Json, None).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("public.product").unwrap().is_array());
}

#[test]
fn table_filter_restricts_output() {
    let out = export(&[snap()], Format::Json, Some("public.nonexistent")).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.as_object().unwrap().len(), 0);
}
