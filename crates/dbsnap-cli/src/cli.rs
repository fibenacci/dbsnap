//! Command-line argument definitions (clap).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "dbsnap",
    version,
    about = "Deterministic database snapshots, diffs and integrity verification"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new dbsnap repository in the current directory.
    Init {
        /// Connection string. Falls back to the DATABASE_URL env var at runtime.
        #[arg(long, env = "DATABASE_URL")]
        database_url: Option<String>,
        /// Database schema to snapshot.
        #[arg(long, default_value = "public")]
        schema: String,
    },
    /// Capture the current database state as a new commit.
    Commit {
        /// Commit message.
        #[arg(short, long)]
        message: String,
        /// Author name (defaults to $USER).
        #[arg(long)]
        author: Option<String>,
    },
    /// Show the commit history.
    Log {
        /// Maximum number of commits to show.
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },
    /// Show HEAD and a summary of changes since the last commit.
    Status,
    /// Show the semantic diff between two commits (default: HEAD~1 HEAD).
    Diff {
        /// Old reference (default HEAD~1).
        old: Option<String>,
        /// New reference (default HEAD).
        new: Option<String>,
        /// Show every changed column value, not just per-table counts.
        #[arg(long)]
        verbose: bool,
        /// Emit machine-readable JSON instead of formatted text.
        #[arg(long)]
        json: bool,
    },
    /// Verify integrity of the stored history (and optionally the live DB).
    Verify {
        /// Also compare HEAD against the current database state.
        #[arg(long)]
        live: bool,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Generate a self-contained HTML report (timeline, diff, integrity).
    Report {
        /// Old reference for the diff section (default HEAD~1).
        old: Option<String>,
        /// New reference for the diff section (default HEAD).
        new: Option<String>,
        /// Write to a file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Export the database state recorded at a commit.
    Export {
        /// Commit to export (default HEAD).
        #[arg(long, default_value = "HEAD")]
        commit: String,
        /// Output format.
        #[arg(long, value_parser = ["json", "sql"], default_value = "json")]
        format: String,
        /// Restrict to a single qualified table (schema.table).
        #[arg(long)]
        table: Option<String>,
        /// Write to a file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}
