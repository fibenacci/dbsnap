//! Human-facing terminal rendering for commits, diffs and verification reports.

use dbsnap_core::{format_unix_utc, Commit, DbHash};
use dbsnap_diff::SnapshotDiff;
use dbsnap_engine::{CommitOutcome, Status};
use dbsnap_integrity::VerifyReport;

const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

pub fn print_commit_outcome(outcome: &CommitOutcome, summary: &str) {
    match outcome {
        CommitOutcome::Unchanged { head } => {
            println!("nothing to commit: database state matches {}", head.short());
        }
        CommitOutcome::Created {
            commit,
            tree,
            tables,
            rows,
        } => {
            println!("[{}] {summary}", commit.short());
            println!("  {tables} tables, {rows} rows, tree {}", tree.short());
        }
    }
}

pub fn print_status(status: &Status) {
    match &status.head {
        None => println!("No commits yet. Run `dbsnap commit -m \"...\"`."),
        Some((hash, commit)) => {
            println!("HEAD {}  {}", hash.short(), commit.summary());
            println!("  {} tables, {} rows", status.tables, status.rows);
        }
    }
}

pub fn print_history(chain: &[(DbHash, Commit)]) {
    for (hash, commit) in chain {
        print_commit(hash, commit);
    }
}

fn print_commit(hash: &DbHash, commit: &Commit) {
    println!("{BOLD}commit {}{RESET}", hash.to_hex());
    println!("  author:  {}", commit.author);
    println!("  date:    {}", format_unix_utc(commit.timestamp));
    println!("  tree:    {}", commit.tree.short());
    if let Some(p) = &commit.parent {
        println!("  parent:  {}", p.short());
    }
    println!();
    for line in commit.message.lines() {
        println!("    {line}");
    }
    println!();
}

pub fn print_diff(diff: &SnapshotDiff, verbose: bool) {
    for table in &diff.added_tables {
        println!("{GREEN}+ table {table}{RESET} (new)");
    }
    for table in &diff.removed_tables {
        println!("{RED}- table {table}{RESET} (removed)");
    }

    for t in &diff.tables {
        println!("{BOLD}Table {}{RESET}", t.table);
        if !t.inserted.is_empty() {
            println!("  {GREEN}+ {} rows inserted{RESET}", t.inserted.len());
        }
        if !t.updated.is_empty() {
            println!("  {YELLOW}~ {} rows updated{RESET}", t.updated.len());
        }
        if !t.deleted.is_empty() {
            println!("  {RED}- {} rows deleted{RESET}", t.deleted.len());
        }

        if verbose {
            for row in &t.updated {
                println!("    {DIM}pk {}{RESET}", row.pk);
                for c in &row.columns {
                    println!(
                        "      {}: {} {DIM}->{RESET} {}",
                        c.column,
                        compact(&c.old),
                        compact(&c.new)
                    );
                }
            }
            for pk in &t.inserted {
                println!("    {GREEN}+ pk {pk}{RESET}");
            }
            for pk in &t.deleted {
                println!("    {RED}- pk {pk}{RESET}");
            }
        }
        println!();
    }
}

pub fn print_verify(report: &VerifyReport) {
    println!(
        "Checked {} commits, {} tables, {} rows.",
        report.commits_checked, report.tables_checked, report.rows_checked
    );
    if report.ok() {
        println!("{GREEN}✓ Integrity verified — hash chain intact.{RESET}");
    } else {
        println!(
            "{RED}✗ {} integrity violation(s) detected:{RESET}",
            report.violations.len()
        );
        for v in &report.violations {
            println!("  {RED}[{}]{RESET} {}", v.kind, v.detail);
        }
    }
}

/// Compact one-line rendering of a JSON value for diff output.
fn compact(v: &serde_json::Value) -> String {
    let s = match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    // Truncate by character, not byte, so multi-byte UTF-8 never panics.
    if s.chars().count() > 60 {
        let mut t: String = s.chars().take(59).collect();
        t.push('…');
        t
    } else {
        s
    }
}
