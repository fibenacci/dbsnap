use anyhow::Result;

use crate::{context, render};

pub async fn run(message: String, author: Option<String>) -> Result<()> {
    let repo = context::open()?;
    let source = context::connect(&repo).await?;
    let author = author
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "unknown".into());

    let summary = message.lines().next().unwrap_or("").to_string();
    let outcome = repo.commit(&source, message, author).await?;
    render::print_commit_outcome(&outcome, &summary);
    Ok(())
}
