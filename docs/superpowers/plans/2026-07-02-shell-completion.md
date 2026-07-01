# Shell Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add dynamic issue-ID tab completion for `git issues show/edit/close/reopen` across zsh, bash, and fish.

**Architecture:** Add a hidden `git issues complete-ids` subcommand that prints current issue IDs (one per line, silently exits 0 when uninitialized), and a public `git issues completions <shell>` subcommand that emits a small shell script hooking into each shell's git-subcommand extension point (`_git_issues` in bash, `_git-issues` in zsh, `__fish_seen_subcommand_from issues` in fish). Those scripts call `git issues complete-ids` to fill the ID argument.

**Tech Stack:** Rust 2021, clap 4 (derive + `ValueEnum`), anyhow. Shell scripts embedded via `include_str!`.

## Global Constraints

- Rust edition 2021; no new runtime dependencies; no new dev-dependencies (tests use `std` only).
- `complete-ids` MUST NOT fail, hang, or produce side effects (no editor, no writes); it always exits 0.
- Invocation form is `git issues …` (a git subcommand); completion hooks target git's per-shell dispatch.
- Embedded shell scripts each call back with the literal string `git issues complete-ids`.
- Follow existing style: small focused modules, `include_str!` for embedded assets (as `SKILL.md` already does in `src/cmd.rs`).

---

### Task 1: `collect_issue_ids` + hidden `complete-ids` subcommand

**Files:**
- Modify: `src/cmd.rs` (add `collect_issue_ids` and `complete_ids`, plus a `#[cfg(test)]` module)
- Modify: `src/main.rs` (add hidden `CompleteIds` variant + dispatch)

**Interfaces:**
- Produces: `pub fn collect_issue_ids(s: &crate::config::Settings) -> Vec<String>` — sorted issue IDs found under `<s.path>/issues/*.md`; empty when the directory is missing/unreadable.
- Produces: `pub fn complete_ids() -> anyhow::Result<()>` — resolves settings and prints `collect_issue_ids` one-per-line to stdout; always returns `Ok(())`.
- Consumes (existing): `crate::config::{Settings, resolve}`, `crate::frontmatter::Document`.

- [ ] **Step 1: Write the failing tests**

Add to the bottom of `src/cmd.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A throwaway worktree dir with an empty `issues/` subdir.
    fn temp_worktree() -> (Settings, PathBuf) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir()
            .join(format!("git-issues-test-{}-{}", std::process::id(), n));
        fs::create_dir_all(base.join("issues")).unwrap();
        let s = Settings {
            branch: "meta/issues".into(),
            path: base.to_string_lossy().into_owned(),
            remote: None,
        };
        (s, base)
    }

    /// Write an issue file. When `id` is Some, emit front-matter carrying it;
    /// otherwise write a body-only file so the ID must fall back to the stem.
    fn write_issue(base: &Path, file_stem: &str, id: Option<&str>) {
        let path = base.join("issues").join(format!("{file_stem}.md"));
        let content = match id {
            Some(id) => format!("---\nid: {id}\ntitle: t\nstatus: open\n---\n\nbody\n"),
            None => "just a body, no front-matter\n".to_string(),
        };
        fs::write(path, content).unwrap();
    }

    #[test]
    fn ids_are_sorted_and_from_frontmatter() {
        let (s, base) = temp_worktree();
        write_issue(&base, "second-file", Some("zeta-aaaa"));
        write_issue(&base, "first-file", Some("alpha-bbbb"));
        // A non-markdown file must be ignored.
        fs::write(base.join("issues").join("notes.txt"), "ignore me").unwrap();

        let ids = collect_issue_ids(&s);
        assert_eq!(ids, vec!["alpha-bbbb".to_string(), "zeta-aaaa".to_string()]);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn missing_issues_dir_yields_empty() {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir()
            .join(format!("git-issues-test-missing-{}-{}", std::process::id(), n));
        let s = Settings {
            branch: "meta/issues".into(),
            path: base.to_string_lossy().into_owned(),
            remote: None,
        };
        assert!(collect_issue_ids(&s).is_empty());
    }

    #[test]
    fn id_falls_back_to_file_stem() {
        let (s, base) = temp_worktree();
        write_issue(&base, "no-frontmatter-1234", None);

        let ids = collect_issue_ids(&s);
        assert_eq!(ids, vec!["no-frontmatter-1234".to_string()]);

        fs::remove_dir_all(&base).ok();
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib collect_issue_ids ids_are_sorted missing_issues id_falls_back 2>&1 | tail -20`
Expected: compile error / FAIL — `collect_issue_ids` is not defined.

- [ ] **Step 3: Implement `collect_issue_ids` and `complete_ids`**

Add to `src/cmd.rs` in the `sync / path / status` region (or just after `list`), before the test module:

```rust
// ---------------------------------------------------------------------------
// completion helpers
// ---------------------------------------------------------------------------

/// Collect issue IDs under `<worktree>/issues/*.md`, sorted.
///
/// Robust by design: a missing/unreadable directory or a malformed file yields
/// an empty list (or is skipped) rather than an error — this feeds shell
/// completion, which must never see a failure.
pub fn collect_issue_ids(s: &Settings) -> Vec<String> {
    let dir = Path::new(&s.path).join("issues");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut ids: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let id = match fs::read_to_string(&path) {
            Ok(raw) => Document::parse(&raw)
                .get("id")
                .map(str::to_string)
                .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned())),
            Err(_) => path.file_stem().map(|s| s.to_string_lossy().into_owned()),
        };
        if let Some(id) = id {
            ids.push(id);
        }
    }
    ids.sort();
    ids
}

/// Hidden subcommand: print issue IDs one per line for shell completion.
/// Never errors — an uninitialized repo simply prints nothing.
pub fn complete_ids() -> Result<()> {
    if let Ok(s) = config::resolve() {
        for id in collect_issue_ids(&s) {
            println!("{id}");
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Wire the hidden subcommand into `src/main.rs`**

In the `Command` enum, add after the `Sync` variant:

```rust
    /// (internal) print issue IDs for shell completion
    #[command(hide = true)]
    CompleteIds,
```

In `main()`'s match, add after the `Command::Sync` arm:

```rust
        Command::CompleteIds => cmd::complete_ids(),
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: PASS (3 new tests green), and `cargo build` succeeds.

- [ ] **Step 6: Sanity-check the hidden command is wired and hidden**

Run: `cargo run -- complete-ids; echo "exit=$?"; cargo run -- --help 2>&1 | grep -c complete-ids`
Expected: `complete-ids` runs and prints `exit=0` even outside an issues worktree (it may print nothing or a "not inside a git repository" is avoided because `config::resolve` errors are swallowed); the `--help` grep prints `0` (command is hidden).

- [ ] **Step 7: Commit**

```bash
git add src/cmd.rs src/main.rs
git commit -m "feat: add hidden complete-ids subcommand and collect_issue_ids"
```

---

### Task 2: `completions <shell>` command + embedded shell scripts

**Files:**
- Create: `src/completions.rs` (the `Shell` enum, `script_for`, `print`, and tests)
- Create: `src/completions/bash`
- Create: `src/completions/zsh`
- Create: `src/completions/fish`
- Modify: `src/main.rs` (declare `mod completions;`, add `Completions { shell }` variant + dispatch)

**Interfaces:**
- Produces: `pub enum Shell { Bash, Zsh, Fish }` (derives `clap::ValueEnum, Clone, Copy`).
- Produces: `pub fn print(shell: Shell) -> anyhow::Result<()>` — prints the embedded script for `shell` to stdout.
- Consumes: nothing from Task 1 at compile time; the emitted scripts call `git issues complete-ids` at runtime.

- [ ] **Step 1: Create the bash script `src/completions/bash`**

```bash
# git issues — bash completion (source after git's own bash completion).
# git's completion calls `_git_issues` for `git issues …`.
_git_issues() {
    local subcommands="init new list show edit close reopen sync path status completions"
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local sub="" i
    for ((i = 2; i < COMP_CWORD; i++)); do
        case "${COMP_WORDS[i]}" in
            init|new|list|show|edit|close|reopen|sync|path|status|completions)
                sub="${COMP_WORDS[i]}"; break;;
        esac
    done

    if [ -z "$sub" ]; then
        COMPREPLY=($(compgen -W "$subcommands" -- "$cur"))
        return
    fi

    case "$sub" in
        show|edit|close|reopen)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "--sync" -- "$cur"))
            else
                COMPREPLY=($(compgen -W "$(git issues complete-ids 2>/dev/null)" -- "$cur"))
            fi
            ;;
        completions)
            COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
            ;;
    esac
}
```

- [ ] **Step 2: Create the zsh script `src/completions/zsh`**

```zsh
#compdef git-issues
# git issues — zsh completion. zsh's _git dispatches to _git-issues for
# `git issues …`; this file also serves the standalone `git-issues` binary.

local curcontext="$curcontext" state line
typeset -A opt_args

local -a subcommands
subcommands=(
    'init:Set up the issues worktree'
    'new:Create a new issue'
    'list:List issues'
    'show:Print an issue'
    'edit:Edit an issue'
    'close:Close an issue'
    'reopen:Reopen an issue'
    'sync:Sync the issues branch'
    'path:Print the worktree path'
    'status:Show ahead/behind'
    'completions:Print a shell completion script'
)

_arguments -C \
    '1: :->subcommand' \
    '*:: :->args' && return

case $state in
    subcommand)
        _describe -t commands 'git issues command' subcommands
        ;;
    args)
        case $line[1] in
            show|edit|close|reopen)
                local -a ids
                ids=(${(f)"$(git issues complete-ids 2>/dev/null)"})
                _describe -t issues 'issue' ids
                ;;
            completions)
                local -a shells
                shells=(bash zsh fish)
                _describe -t shells 'shell' shells
                ;;
        esac
        ;;
esac
```

- [ ] **Step 3: Create the fish script `src/completions/fish`**

```fish
# git issues — fish completion.

# Disable file completion once `issues` is on the command line.
complete -c git -n '__fish_seen_subcommand_from issues' -f

# Subcommand names (only before a sub-subcommand is chosen).
complete -c git -n '__fish_seen_subcommand_from issues; and not __fish_seen_subcommand_from init new list show edit close reopen sync path status completions' \
    -a 'init new list show edit close reopen sync path status completions'

# Dynamic issue IDs for the ID-taking commands.
complete -c git -n '__fish_seen_subcommand_from issues; and __fish_seen_subcommand_from show edit close reopen' \
    -a '(git issues complete-ids 2>/dev/null)'

# Shell names for `completions`.
complete -c git -n '__fish_seen_subcommand_from issues; and __fish_seen_subcommand_from completions' \
    -a 'bash zsh fish'
```

- [ ] **Step 4: Write the failing test for `src/completions.rs`**

Create `src/completions.rs` with ONLY the enum, `script_for`, and tests first (so the test compiles and fails on the missing `print` wiring only later). Write:

```rust
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
```

- [ ] **Step 5: Run the test to verify it fails**

Run: `cargo test --lib every_script_is_nonempty 2>&1 | tail -20`
Expected: FAIL — `src/completions.rs` is not yet a module of the crate (`mod completions;` missing in `main.rs`), so it isn't compiled/run.

- [ ] **Step 6: Wire the module and public command into `src/main.rs`**

Add near the other `mod` lines at the top of `src/main.rs`:

```rust
mod completions;
```

In the `Command` enum, add after the `Sync` variant (and before the hidden `CompleteIds` from Task 1):

```rust
    /// Print a shell completion script (bash, zsh, or fish)
    Completions {
        /// Target shell
        #[arg(value_enum)]
        shell: completions::Shell,
    },
```

In `main()`'s match, add after the `Command::Sync` arm:

```rust
        Command::Completions { shell } => completions::print(shell),
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: PASS (all tests, including the new completions test).

- [ ] **Step 8: Verify each script emits and references the callback**

Run: `for s in bash zsh fish; do echo "== $s =="; cargo run -q -- completions $s | grep -c 'git issues complete-ids'; done`
Expected: each shell prints a count `>= 1`.

- [ ] **Step 9: Commit**

```bash
git add src/completions.rs src/completions/bash src/completions/zsh src/completions/fish src/main.rs
git commit -m "feat: add completions subcommand emitting bash/zsh/fish scripts"
```

---

### Task 3: Install documentation

**Files:**
- Modify: `src/main.rs` (add a `long_about` to the `Completions` variant with install instructions)

**Interfaces:**
- Consumes: the `Completions` variant from Task 2.

- [ ] **Step 1: Add install instructions as the command's long help**

Replace the `Completions` variant's doc comment in `src/main.rs` with a `long_about` (keep the short line for `--help` listings):

```rust
    /// Print a shell completion script (bash, zsh, or fish)
    ///
    /// Install:
    ///   zsh:  git issues completions zsh  > "${fpath[1]}/_git-issues"   # then: compinit
    ///   bash: git issues completions bash > ~/.git-issues-completion.bash
    ///         # then source it AFTER git's completion in ~/.bashrc:
    ///         #   source ~/.git-issues-completion.bash
    ///   fish: git issues completions fish > ~/.config/fish/completions/git-issues.fish
    ///
    /// Completes issue IDs for `show`, `edit`, `close`, and `reopen`.
    Completions {
        /// Target shell
        #[arg(value_enum)]
        shell: completions::Shell,
    },
```

- [ ] **Step 2: Verify the long help renders**

Run: `cargo run -q -- completions --help`
Expected: output includes the `Install:` block and lines for zsh, bash, and fish.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "docs: document shell completion install steps in completions --help"
```

---

### Task 4: Manual end-to-end verification (per shell)

**Files:** none (verification only). This task is a checklist for a human/agent with an interactive shell; it produces no commit.

**Interfaces:**
- Consumes: the built binary (`cargo build` / a repo with `git issues init` run and at least one issue created).

- [ ] **Step 1: Prepare a repo with issues**

In a scratch git repo on `PATH`-accessible `git-issues`:
Run: `git issues init && git issues new "First test issue" && git issues new "Second test issue"`
Expected: two issues created; `git issues list` shows both IDs.

- [ ] **Step 2: zsh**

Run: `git issues completions zsh > "${fpath[1]}/_git-issues" && compinit`
Then type: `git issues show <TAB>`
Expected: both issue IDs are offered as candidates.

- [ ] **Step 3: bash**

Run: `git issues completions bash > /tmp/gi.bash && source /usr/share/bash-completion/completions/git 2>/dev/null; source /tmp/gi.bash`
Then type: `git issues edit <TAB>`
Expected: both issue IDs are offered.

- [ ] **Step 4: fish**

Run: `git issues completions fish > ~/.config/fish/completions/git-issues.fish`
Then in a new fish session type: `git issues close <TAB>`
Expected: both issue IDs are offered.

- [ ] **Step 5: Uninitialized-safety check**

In a directory that is NOT a git repo:
Run: `git issues complete-ids; echo "exit=$?"`
Expected: no output, `exit=0` (completion degrades to nothing, never an error).
