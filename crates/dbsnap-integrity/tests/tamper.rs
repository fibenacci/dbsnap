//! End-to-end integrity tests that don't need a live database: build a real
//! `.dbsnap` store, then tamper with stored objects on disk and assert that
//! `verify_chain` catches it.

use std::fs;

use dbsnap_core::{make_record, Column, Commit, TableSchema, TableSnapshot, Tree};
use dbsnap_integrity::verify_chain;
use dbsnap_storage::{Config, Store};
use serde_json::json;

fn schema() -> TableSchema {
    TableSchema {
        schema: "public".into(),
        name: "customer".into(),
        columns: vec![
            Column {
                name: "id".into(),
                data_type: "integer".into(),
                nullable: false,
                ordinal: 1,
                is_primary_key: true,
            },
            Column {
                name: "email".into(),
                data_type: "text".into(),
                nullable: false,
                ordinal: 2,
                is_primary_key: false,
            },
        ],
        primary_key: vec!["id".into()],
    }
}

/// Build a store with one commit capturing one customer row. Returns the store
/// and the stored table object's path.
fn build_repo() -> (tempfile::TempDir, Store, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::init(tmp.path(), Config::default()).unwrap();

    let s = schema();
    let snap = TableSnapshot {
        schema: s.clone(),
        rows: vec![make_record(&s, json!({"id": 1, "email": "a@example.com"}))],
    };
    let table_hash = store.write_table_snapshot(&snap).unwrap();
    let tree = Tree::from_snapshots(&[snap]);
    let tree_hash = store.write_tree(&tree).unwrap();
    let commit = Commit {
        tree: tree_hash,
        parent: None,
        message: "seed".into(),
        timestamp: 1,
        author: "t".into(),
    };
    let head = store.write_commit(&commit).unwrap();
    store.set_head(&head).unwrap();

    let path = store.root.join("tables").join(format!("{table_hash}.zst"));
    (tmp, store, path)
}

#[test]
fn clean_repo_verifies() {
    let (_tmp, store, _) = build_repo();
    let report = verify_chain(&store).unwrap();
    assert!(report.ok(), "violations: {:?}", report.violations);
    assert_eq!(report.commits_checked, 1);
    assert_eq!(report.rows_checked, 1);
}

#[test]
fn detects_tampered_row_value() {
    let (_tmp, store, path) = build_repo();

    // Edit the row's value on disk WITHOUT updating its stored hash — exactly
    // what an out-of-band edit of snapshot history would look like.
    let compressed = fs::read(&path).unwrap();
    let mut obj: serde_json::Value =
        serde_json::from_slice(&zstd::decode_all(&compressed[..]).unwrap()).unwrap();
    obj["rows"][0]["data"]["email"] = json!("attacker@evil.com");
    let tampered = zstd::encode_all(serde_json::to_vec(&obj).unwrap().as_slice(), 3).unwrap();
    fs::write(&path, tampered).unwrap();

    let report = verify_chain(&store).unwrap();
    assert!(!report.ok(), "tampering should be detected");
    let kinds: Vec<&str> = report.violations.iter().map(|v| v.kind.as_str()).collect();
    // Hash mismatch surfaces at the row level (and cascades to table/tree).
    assert!(kinds.contains(&"row-hash-mismatch"), "kinds: {kinds:?}");
}
