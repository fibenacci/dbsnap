//! `dbsnap` — deterministic database snapshots, diffs and integrity verification.
//!
//! This binary is intentionally thin: it parses arguments ([`cli`]), wires a
//! concrete database [`context`], delegates the actual work to
//! `dbsnap-engine`'s `Repository`, and renders the result ([`render`]). All
//! domain logic lives in the library crates.

mod cli;
mod commands;
mod context;
mod datetime;
mod render;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    init_tracing();
    if let Err(e) = run().await {
        eprintln!("\x1b[31merror:\x1b[0m {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    match Cli::parse().command {
        Command::Init { database_url, schema } => commands::init::run(database_url, schema),
        Command::Commit { message, author } => commands::commit::run(message, author).await,
        Command::Log { limit } => commands::log::run(limit),
        Command::Status => commands::status::run().await,
        Command::Diff { old, new, verbose, json } => commands::diff::run(old, new, verbose, json),
        Command::Verify { live, json } => commands::verify::run(live, json).await,
        Command::Export { commit, format, table, output } => {
            commands::export::run(commit, format, table, output)
        }
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_target(false)
        .without_time()
        .init();
}
