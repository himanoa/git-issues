//! Per-repository settings, stored in `.git/config` under the `issues` section.

use std::path::Path;

use anyhow::{Context, Result};

use crate::git;

pub const DEFAULT_BRANCH: &str = "meta/issues";

/// Resolved settings for the current repository.
pub struct Settings {
    /// The issues branch name (`issues.branch`).
    pub branch: String,
    /// Absolute path to the issues worktree (`issues.worktreepath`).
    pub path: String,
    /// Remote to sync against, if any (prefers `origin`).
    pub remote: Option<String>,
}

/// Read a `git config` value, or `None` if unset/empty.
pub fn get(key: &str) -> Option<String> {
    let out = git::output(&["config", "--get", key]).ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// Write a `git config` value into the repository config.
pub fn set(key: &str, value: &str) -> Result<()> {
    git::capture(&["config", key, value]).map(|_| ())
}

/// The repository's top-level directory.
pub fn repo_root() -> Result<String> {
    git::capture(&["rev-parse", "--show-toplevel"]).context("not inside a git repository")
}

/// Default worktree path: a sibling directory `../<repo-name>-issues`.
pub fn default_path(root: &str) -> String {
    let root = Path::new(root);
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string());
    root.parent()
        .unwrap_or(root)
        .join(format!("{name}-issues"))
        .to_string_lossy()
        .into_owned()
}

/// Pick a remote to sync with: `origin` if present, otherwise the first one.
fn detect_remote() -> Option<String> {
    let listing = git::capture(&["remote"]).ok()?;
    let remotes: Vec<&str> = listing.lines().filter(|l| !l.is_empty()).collect();
    if remotes.contains(&"origin") {
        Some("origin".to_string())
    } else {
        remotes.first().map(|s| s.to_string())
    }
}

/// Resolve effective settings from config, falling back to defaults.
pub fn resolve() -> Result<Settings> {
    let root = repo_root()?;
    Ok(Settings {
        branch: get("issues.branch").unwrap_or_else(|| DEFAULT_BRANCH.to_string()),
        path: get("issues.worktreepath").unwrap_or_else(|| default_path(&root)),
        remote: detect_remote(),
    })
}
