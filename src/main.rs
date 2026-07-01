//! `git-issues` — track issues as Markdown files on an orphan branch, kept in a
//! dedicated worktree so they sync via `git push`/`pull` like anything else.

mod cmd;
mod config;
mod frontmatter;
mod git;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// File-based issue tracking on a dedicated orphan branch.
///
/// Defaults are stored per-repo in .git/config as issues.branch /
/// issues.worktreepath.
#[derive(Parser)]
#[command(name = "git issues", bin_name = "git issues", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Set up (or reconnect) the issues worktree
    Init {
        /// Issues branch name (default: meta/issues)
        #[arg(long)]
        branch: Option<String>,
        /// Worktree path (default: ../<repo>-issues)
        #[arg(long)]
        path: Option<String>,
    },
    /// Create a new issue and open your editor
    New {
        /// Issue title (may be several words)
        #[arg(required = true, num_args = 1.., value_name = "TITLE")]
        title: Vec<String>,
        /// Push the issues branch right after creating
        #[arg(long)]
        sync: bool,
    },
    /// List issues, optionally filtered
    List {
        /// Only show issues with this status
        #[arg(long)]
        status: Option<String>,
        /// Only show issues carrying this label
        #[arg(long)]
        label: Option<String>,
    },
    /// Print an issue
    Show {
        /// Issue ID
        id: String,
    },
    /// Edit an issue in your editor
    Edit {
        /// Issue ID
        id: String,
        #[arg(long)]
        sync: bool,
    },
    /// Mark an issue as closed
    Close {
        /// Issue ID
        id: String,
        #[arg(long)]
        sync: bool,
    },
    /// Mark an issue as open
    Reopen {
        /// Issue ID
        id: String,
        #[arg(long)]
        sync: bool,
    },
    /// fetch + merge + push the issues branch
    Sync,
    /// Print the issues worktree path
    Path,
    /// Show ahead/behind of the issues branch
    Status,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Init { branch, path } => cmd::init(branch, path),
        Command::New { title, sync } => cmd::new(&title, sync),
        Command::List { status, label } => cmd::list(status, label),
        Command::Show { id } => cmd::show(&id),
        Command::Edit { id, sync } => cmd::edit(&id, sync),
        Command::Close { id, sync } => cmd::set_status(&id, "closed", sync),
        Command::Reopen { id, sync } => cmd::set_status(&id, "open", sync),
        Command::Sync => cmd::sync(),
        Command::Path => cmd::path(),
        Command::Status => cmd::status(),
    }
}
