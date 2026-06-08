use anyhow::Result;

use crate::{context, render};

pub async fn run() -> Result<()> {
    let repo = context::open()?;
    let status = repo.status()?;
    render::print_status(&status);

    // If a database is reachable and we have a HEAD, show uncommitted changes.
    if status.head.is_some() {
        if let Ok(source) = context::connect(&repo).await {
            match repo.live_diff(&source).await {
                Ok(diff) if diff.is_empty() => println!("\nWorking database matches HEAD."),
                Ok(diff) => {
                    println!("\nUncommitted changes in the live database:");
                    render::print_diff(&diff, false);
                }
                Err(e) => println!("\n(could not inspect live database: {e})"),
            }
        }
    }
    Ok(())
}
