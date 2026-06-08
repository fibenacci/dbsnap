//! Behavioural tests for the deterministic hash hierarchy (public API only).

use dbsnap_core::{make_record, row_pk, Column, Commit, TableSchema, TableSnapshot, Tree};
use serde_json::json;

fn sample_schema() -> TableSchema {
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

#[test]
fn table_hash_is_order_independent() {
    let s = sample_schema();
    let r1 = make_record(&s, json!({"id": 1, "price": "9.99"}));
    let r2 = make_record(&s, json!({"id": 2, "price": "19.99"}));

    let a = TableSnapshot { schema: s.clone(), rows: vec![r1.clone(), r2.clone()] };
    let b = TableSnapshot { schema: s.clone(), rows: vec![r2, r1] };
    assert_eq!(a.table_hash(), b.table_hash());
}

#[test]
fn table_hash_changes_with_data() {
    let s = sample_schema();
    let a = TableSnapshot { schema: s.clone(), rows: vec![make_record(&s, json!({"id": 1, "price": "9.99"}))] };
    let b = TableSnapshot { schema: s.clone(), rows: vec![make_record(&s, json!({"id": 1, "price": "8.99"}))] };
    assert_ne!(a.table_hash(), b.table_hash());
}

#[test]
fn commit_chain_is_tamper_evident() {
    let s = sample_schema();
    let snap = TableSnapshot { schema: s.clone(), rows: vec![make_record(&s, json!({"id": 1, "price": "9.99"}))] };
    let tree = Tree::from_snapshots(&[snap]);

    let root = Commit { tree: tree.hash(), parent: None, message: "init".into(), timestamp: 100, author: "t".into() };
    let child = Commit { tree: tree.hash(), parent: Some(root.hash()), message: "next".into(), timestamp: 200, author: "t".into() };

    // Tampering with the root's message changes its hash, breaking the link.
    let mut tampered = root.clone();
    tampered.message = "evil".into();
    assert_ne!(tampered.hash(), child.parent.unwrap());
}

#[test]
fn tree_total_rows_sums_entries() {
    let s = sample_schema();
    let snap = TableSnapshot {
        schema: s.clone(),
        rows: vec![
            make_record(&s, json!({"id": 1, "price": "1.00"})),
            make_record(&s, json!({"id": 2, "price": "2.00"})),
        ],
    };
    assert_eq!(Tree::from_snapshots(&[snap]).total_rows(), 2);
}

#[test]
fn pk_identity_uses_key_columns() {
    let s = sample_schema();
    assert_eq!(
        row_pk(&s, &json!({"id": 7, "price": "1.00"})),
        row_pk(&s, &json!({"id": 7, "price": "2.00"}))
    );
}
