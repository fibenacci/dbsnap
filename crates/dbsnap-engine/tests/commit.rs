//! Engine workflow tests driven by an in-memory fake source — no database
//! required. This is exactly the payoff of the `SnapshotSource` abstraction.

use std::cell::RefCell;

use anyhow::Result;
use dbsnap_core::{make_record, Column, SnapshotSource, TableSchema, TableSnapshot};
use dbsnap_engine::{CommitOutcome, RepoConfig, Repository};
use serde_json::{json, Value};

/// A source that hands out a scripted sequence of states, one per `capture`.
struct FakeSource {
    states: RefCell<std::collections::VecDeque<Vec<TableSnapshot>>>,
}

impl FakeSource {
    fn new(states: Vec<Vec<TableSnapshot>>) -> Self {
        FakeSource { states: RefCell::new(states.into()) }
    }
}

impl SnapshotSource for FakeSource {
    async fn capture(&self, _schema: &str) -> Result<Vec<TableSnapshot>> {
        // Repeat the last state once exhausted (models an unchanging DB).
        let mut states = self.states.borrow_mut();
        if states.len() > 1 {
            Ok(states.pop_front().unwrap())
        } else {
            Ok(states.front().cloned().unwrap_or_default())
        }
    }
}

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

fn state(rows: Vec<Value>) -> Vec<TableSnapshot> {
    let s = schema();
    vec![TableSnapshot { schema: s.clone(), rows: rows.into_iter().map(|r| make_record(&s, r)).collect() }]
}

#[tokio::test]
async fn full_workflow() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = Repository::init(tmp.path(), RepoConfig::default()).unwrap();

    let source = FakeSource::new(vec![
        state(vec![json!({"id": 1, "price": "9.99"})]),
        state(vec![json!({"id": 1, "price": "8.99"}), json!({"id": 2, "price": "1.00"})]),
    ]);

    // First commit: root.
    let first = repo.commit(&source, "init".into(), "test".into()).await.unwrap();
    assert!(matches!(first, CommitOutcome::Created { rows: 1, .. }));

    // Second commit: price change + insert.
    let second = repo.commit(&source, "update".into(), "test".into()).await.unwrap();
    assert!(matches!(second, CommitOutcome::Created { rows: 2, .. }));

    // Third capture sees the same (last) state => no-op commit.
    let third = repo.commit(&source, "noop".into(), "test".into()).await.unwrap();
    assert!(matches!(third, CommitOutcome::Unchanged { .. }));

    // History has exactly two commits.
    assert_eq!(repo.history(None).unwrap().len(), 2);

    // Diff HEAD~1..HEAD reflects the change.
    let diff = repo.diff("HEAD~1", "HEAD").unwrap();
    assert_eq!(diff.tables.len(), 1);
    let t = &diff.tables[0];
    assert_eq!(t.inserted.len(), 1);
    assert_eq!(t.updated.len(), 1);

    // Integrity holds.
    let report = repo.verify().unwrap();
    assert!(report.ok(), "violations: {:?}", report.violations);
    assert_eq!(report.commits_checked, 2);
}
