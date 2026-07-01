//! `git issues completions <shell>` — emit a shell completion script.

use anyhow::Result;
use clap::ValueEnum;

/// A shell we can emit completion for.
#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

/// The embedded completion script for a shell.
fn script_for(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => include_str!("completions/bash"),
        Shell::Zsh => include_str!("completions/zsh"),
        Shell::Fish => include_str!("completions/fish"),
    }
}

/// Print the completion script for `shell` to stdout.
pub fn print(shell: Shell) -> Result<()> {
    print!("{}", script_for(shell));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_script_is_nonempty_and_calls_back() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let script = script_for(shell);
            assert!(!script.trim().is_empty(), "script should not be empty");
            assert!(
                script.contains("git issues complete-ids"),
                "script must call back into `git issues complete-ids`"
            );
        }
    }
}
