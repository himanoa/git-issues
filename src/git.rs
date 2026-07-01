//! Thin wrappers around the `git` executable.

use std::process::{Command, Output};

use anyhow::{bail, Context, Result};

/// Run `git` with the given args, capturing stdout/stderr.
pub fn output(args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn `git {}`", args.join(" ")))
}

/// Run `git`, requiring success, and return trimmed stdout.
pub fn capture(args: &[&str]) -> Result<String> {
    let out = output(args)?;
    if !out.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Run `git` inheriting the parent's stdio (for interactive/streaming commands).
/// Returns whether the command exited successfully.
pub fn run(args: &[&str]) -> Result<bool> {
    let status = Command::new("git")
        .args(args)
        .status()
        .with_context(|| format!("failed to spawn `git {}`", args.join(" ")))?;
    Ok(status.success())
}
