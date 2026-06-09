use std::path::PathBuf;

use anyhow::Result;
use dbsnap_core::{format_unix_utc, now_unix};
use dbsnap_report::{render, DiffView, ReportInput};

use crate::context;

pub fn run(old: Option<String>, new: Option<String>, output: Option<PathBuf>) -> Result<()> {
    let repo = context::open()?;
    let history = repo.history(None)?;
    let verify = repo.verify()?;
    let head = history.first().map(|(h, _)| h);

    // Diff section: HEAD~1..HEAD by default; included when a range is possible.
    let want_diff = history.len() >= 2 || old.is_some() || new.is_some();
    let old_ref = old.unwrap_or_else(|| "HEAD~1".into());
    let new_ref = new.unwrap_or_else(|| "HEAD".into());
    let diff = if want_diff {
        Some(repo.diff(&old_ref, &new_ref)?)
    } else {
        None
    };
    let diff_view = diff.as_ref().map(|d| DiffView {
        old_ref: &old_ref,
        new_ref: &new_ref,
        diff: d,
    });

    let generated_at = format_unix_utc(now_unix());

    let html = render(&ReportInput {
        generated_at: &generated_at,
        head,
        history: &history,
        diff: diff_view,
        verify: &verify,
    });

    match output {
        Some(path) => {
            std::fs::write(&path, html)?;
            println!("Wrote report to {}", path.display());
        }
        None => print!("{html}"),
    }
    Ok(())
}
