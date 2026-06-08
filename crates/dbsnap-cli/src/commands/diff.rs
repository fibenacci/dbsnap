use anyhow::Result;

use crate::{context, render};

pub fn run(old: Option<String>, new: Option<String>, verbose: bool, json: bool) -> Result<()> {
    let repo = context::open()?;
    let old_ref = old.unwrap_or_else(|| "HEAD~1".into());
    let new_ref = new.unwrap_or_else(|| "HEAD".into());

    let diff = repo.diff(&old_ref, &new_ref)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else if diff.is_empty() {
        println!("No changes between {old_ref} and {new_ref}.");
    } else {
        render::print_diff(&diff, verbose);
    }
    Ok(())
}
