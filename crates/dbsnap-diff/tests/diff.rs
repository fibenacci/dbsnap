//! Behavioural tests for the diff engine (public API only).

use dbsnap_core::{make_record, Column, TableSchema, TableSnapshot};
use dbsnap_diff::{diff_snapshots, diff_tables};
use serde_json::json;

fn schema() -> TableSchema {
    TableSchema {
        schema: "public".into(),
        name: "product".into(),
        columns: vec![
            Column { name: "id".into(), data_type: "integer".into(), nullable: false, ordinal: 1, is_primary_key: true },
            Column { name: "price".into(), data_type: "numeric".into(), nullable: true, ordinal: 2, is_primary_key: false },
        ],
        primary_key: vec!["id".into()],
    }
}

fn snap(rows: Vec<serde_json::Value>) -> TableSnapshot {
    let s = schema();
    TableSnapshot { schema: s.clone(), rows: rows.into_iter().map(|r| make_record(&s, r)).collect() }
}

#[test]
fn detects_insert_update_delete() {
    let old = snap(vec![json!({"id": 1, "price": "9.99"}), json!({"id": 2, "price": "5.00"})]);
    let new = snap(vec![json!({"id": 1, "price": "8.99"}), json!({"id": 3, "price": "1.00"})]);

    let d = diff_tables(&old, &new);
    assert_eq!(d.inserted.len(), 1, "id 3 inserted");
    assert_eq!(d.deleted.len(), 1, "id 2 deleted");
    assert_eq!(d.updated.len(), 1, "id 1 updated");

    let change = &d.updated[0];
    assert_eq!(change.columns.len(), 1);
    assert_eq!(change.columns[0].column, "price");
}

#[test]
fn identical_snapshots_have_no_diff() {
    let a = snap(vec![json!({"id": 1, "price": "9.99"})]);
    let b = snap(vec![json!({"id": 1, "price": "9.99"})]);
    assert!(diff_tables(&a, &b).is_empty());
}

#[test]
fn detects_added_and_removed_tables() {
    let old = vec![snap(vec![json!({"id": 1, "price": "1.00"})])];
    let new: Vec<TableSnapshot> = vec![];
    let d = diff_snapshots(&old, &new);
    assert_eq!(d.removed_tables, vec!["public.product".to_string()]);
    assert!(d.added_tables.is_empty());
}
