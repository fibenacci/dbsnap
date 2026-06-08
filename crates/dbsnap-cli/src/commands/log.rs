use anyhow::Result;

use crate::{context, render};

pub fn run(limit: Option<usize>) -> Result<()> {
    let repo = context::open()?;
    let history = repo.history(limit)?;
    if history.is_empty() {
        println!("no commits yet");
        return Ok(());
    }
    render::print_history(&history);
    Ok(())
}
