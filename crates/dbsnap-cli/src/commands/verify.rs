use anyhow::Result;

use crate::{context, render};

pub async fn run(live: bool, json: bool) -> Result<()> {
    let repo = context::open()?;
    let report = repo.verify()?;

    let mut live_diff = None;
    if live {
        let source = context::connect(&repo).await?;
        live_diff = Some(repo.live_diff(&source).await?);
    }

    if json {
        let out = serde_json::json!({ "chain": report, "live": live_diff });
        println!("{}", serde_json::to_string_pretty(&out)?);
        if !report.ok() || live_diff.as_ref().is_some_and(|d| !d.is_empty()) {
            std::process::exit(2);
        }
        return Ok(());
    }

    render::print_verify(&report);
    let mut failed = !report.ok();

    if let Some(diff) = live_diff {
        if diff.is_empty() {
            println!("\nLive database matches HEAD — no out-of-band mutations detected.");
        } else {
            failed = true;
            println!(
                "\n\x1b[31mLive database differs from HEAD\x1b[0m — possible out-of-band mutation:"
            );
            render::print_diff(&diff, false);
        }
    }

    if failed {
        std::process::exit(2);
    }
    Ok(())
}
