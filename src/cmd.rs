//! Subcommand implementations.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::config::{self, Settings};
use crate::frontmatter::Document;
use crate::git;
use crate::util;

/// Minimum git version that provides `worktree add --orphan`.
const MIN_GIT: (u32, u32) = (2, 42);

/// The Claude skill installed into a repo on `init`, embedded at compile time.
const SKILL_MD: &str = include_str!("../skills/git-issues/SKILL.md");

/// Where the skill lands, relative to the repository root.
const SKILL_REL: &str = ".claude/skills/git-issues/SKILL.md";

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn ensure_version() -> Result<()> {
    let raw = git::capture(&["--version"])?; // e.g. "git version 2.43.0"
    let mut parts = raw
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .split('.')
        .filter_map(|s| s.parse::<u32>().ok());
    let version = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    if version >= MIN_GIT {
        Ok(())
    } else {
        bail!(
            "git >= {}.{} is required for `worktree add --orphan`; found: {raw}",
            MIN_GIT.0,
            MIN_GIT.1
        );
    }
}

/// Make a possibly-relative path absolute (relative to the current directory).
fn absolutize(path: &str) -> Result<String> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Ok(p.to_string_lossy().into_owned());
    }
    let cwd = std::env::current_dir().context("cannot determine current directory")?;
    Ok(cwd.join(p).to_string_lossy().into_owned())
}

/// Whether the issues worktree exists (a linked worktree has a `.git` file).
fn is_initialized(s: &Settings) -> bool {
    Path::new(&s.path).join(".git").exists()
}

fn ensure_worktree(s: &Settings) -> Result<()> {
    if is_initialized(s) {
        Ok(())
    } else {
        bail!(
            "issues worktree not found at {}\nRun `git issues init` first.",
            s.path
        );
    }
}

fn issue_id(id: &str) -> &str {
    id.strip_suffix(".md").unwrap_or(id)
}

fn issue_path(s: &Settings, id: &str) -> PathBuf {
    Path::new(&s.path)
        .join("issues")
        .join(format!("{}.md", issue_id(id)))
}

/// `git -C <worktree> add <files> && git -C <worktree> commit -m <msg>`.
fn commit(path: &str, files: &[&str], message: &str) -> Result<()> {
    let mut add = vec!["-C", path, "add"];
    add.extend_from_slice(files);
    git::capture(&add)?;
    git::capture(&["-C", path, "commit", "-m", message])?;
    Ok(())
}

/// Launch the user's git editor (same resolution git uses for commit messages).
fn open_editor(file: &Path) -> Result<()> {
    let editor = git::capture(&["var", "GIT_EDITOR"])?;
    let file = file.to_string_lossy().into_owned();
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh") // $0
        .arg(&file) // $1
        .status()
        .with_context(|| format!("failed to launch editor ({editor})"))?;
    if !status.success() {
        bail!("editor exited with a non-zero status");
    }
    Ok(())
}

fn read_doc(file: &Path) -> Result<Document> {
    let raw = fs::read_to_string(file)
        .with_context(|| format!("cannot read {}", file.display()))?;
    Ok(Document::parse(&raw))
}

fn write_doc(file: &Path, doc: &Document) -> Result<()> {
    fs::write(file, doc.render()).with_context(|| format!("cannot write {}", file.display()))
}

/// Resolve the file for an issue, erroring if it doesn't exist.
fn existing_issue(s: &Settings, id: &str) -> Result<PathBuf> {
    let file = issue_path(s, id);
    if file.exists() {
        Ok(file)
    } else {
        bail!("no such issue: {id}");
    }
}

// ---------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------

/// Write the bundled Claude skill into `<root>/.claude/skills/git-issues/`.
/// Existing files are left untouched so local edits survive re-runs.
fn install_skill(root: &str) -> Result<()> {
    let file = Path::new(root).join(SKILL_REL);
    if file.exists() {
        return Ok(());
    }
    let dir = file.parent().expect("SKILL_REL always has a parent");
    fs::create_dir_all(dir)
        .with_context(|| format!("cannot create {}", dir.display()))?;
    fs::write(&file, SKILL_MD).with_context(|| format!("cannot write {}", file.display()))?;
    println!("Installed Claude skill to {SKILL_REL}");
    Ok(())
}

pub fn init(branch: Option<String>, path: Option<String>, no_skill: bool) -> Result<()> {
    ensure_version()?;
    let root = config::repo_root()?; // fail early if not in a repo

    if !no_skill {
        install_skill(&root)?;
    }

    if let Some(branch) = branch {
        config::set("issues.branch", &branch)?;
    }
    if let Some(path) = path {
        config::set("issues.worktreepath", &absolutize(&path)?)?;
    }

    let s = config::resolve()?;

    if is_initialized(&s) {
        println!("Already initialized: branch '{}' at {}", s.branch, s.path);
        return Ok(());
    }
    if Path::new(&s.path).exists() {
        bail!(
            "{} already exists but is not a git worktree; remove it or choose another --path",
            s.path
        );
    }

    // Pull the latest state of the issues branch so we can attach rather than
    // fork a fresh, unrelated history. Captured (not inherited) because a missing
    // remote branch is expected on first setup and shouldn't print a scary fatal.
    if let Some(remote) = &s.remote {
        let _ = git::output(&["fetch", remote, &s.branch]);
    }

    // Try to attach to an existing branch (local, or DWIM from a remote-tracking
    // branch). This one line covers "reconnect on another machine".
    let attach = git::output(&["worktree", "add", &s.path, &s.branch])?;
    if attach.status.success() {
        println!("Attached existing branch '{}' at {}", s.branch, s.path);
        return Ok(());
    }

    // Nothing to attach to — start a brand new orphan history. Use `-b` (never
    // `-B`) so we abort instead of clobbering an existing branch.
    let create = git::output(&["worktree", "add", "--orphan", "-b", &s.branch, &s.path])?;
    if !create.status.success() {
        bail!(
            "failed to create issues worktree.\n  attach attempt: {}\n  orphan attempt: {}",
            String::from_utf8_lossy(&attach.stderr).trim(),
            String::from_utf8_lossy(&create.stderr).trim(),
        );
    }

    let issues_dir = Path::new(&s.path).join("issues");
    fs::create_dir_all(&issues_dir).context("cannot create issues/")?;
    fs::write(issues_dir.join(".gitkeep"), b"").context("cannot seed issues/")?;
    commit(&s.path, &["issues/.gitkeep"], "Initialize git-issues")?;
    println!("Created issues branch '{}' at {}", s.branch, s.path);
    Ok(())
}

// ---------------------------------------------------------------------------
// new
// ---------------------------------------------------------------------------

pub fn new(title: &[String], sync: bool) -> Result<()> {
    let s = config::resolve()?;
    ensure_worktree(&s)?;

    let title = title.join(" ");
    let title = title.trim();
    if title.is_empty() {
        bail!("an issue title is required");
    }

    let id = util::make_id(title);
    let now = util::utc_now();
    let doc = Document {
        fields: vec![
            ("id".into(), id.clone()),
            ("title".into(), title.into()),
            ("status".into(), "open".into()),
            ("labels".into(), String::new()),
            ("created".into(), now.clone()),
            ("updated".into(), now),
        ],
        body: String::new(),
    };

    let rel = format!("issues/{id}.md");
    let file = Path::new(&s.path).join(&rel);
    write_doc(&file, &doc)?;

    open_editor(&file)?;
    commit(&s.path, &[&rel], &format!("issues: add {id}"))?;
    println!("Created {id}");

    if sync {
        sync_worktree(&s)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

pub fn list(status: Option<String>, label: Option<String>) -> Result<()> {
    let s = config::resolve()?;
    ensure_worktree(&s)?;

    let dir = Path::new(&s.path).join("issues");
    let entries = fs::read_dir(&dir).with_context(|| format!("cannot read {}", dir.display()))?;

    let mut rows: Vec<(String, String, String)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let doc = read_doc(&path)?;
        let id = doc
            .get("id")
            .map(str::to_string)
            .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let issue_status = doc.get("status").unwrap_or_default().to_string();
        let title = doc.get("title").unwrap_or_default().to_string();
        let labels = doc.get("labels").unwrap_or_default();

        if let Some(want) = &status {
            if &issue_status != want {
                continue;
            }
        }
        if let Some(want) = &label {
            let matches = labels
                .split([',', ' '])
                .map(str::trim)
                .any(|l| l == want);
            if !matches {
                continue;
            }
        }
        rows.push((id, issue_status, title));
    }

    rows.sort();
    if rows.is_empty() {
        println!("No issues found.");
        return Ok(());
    }
    let id_w = rows.iter().map(|(id, ..)| id.len()).max().unwrap_or(2);
    let st_w = rows.iter().map(|(_, st, _)| st.len()).max().unwrap_or(6);
    for (id, status, title) in rows {
        println!("{id:<id_w$}  {status:<st_w$}  {title}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

pub fn show(id: &str) -> Result<()> {
    let s = config::resolve()?;
    ensure_worktree(&s)?;
    let file = existing_issue(&s, id)?;
    let raw = fs::read_to_string(&file).with_context(|| format!("cannot read {}", file.display()))?;
    print!("{raw}");
    Ok(())
}

// ---------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------

pub fn edit(id: &str, sync: bool) -> Result<()> {
    let s = config::resolve()?;
    ensure_worktree(&s)?;
    let file = existing_issue(&s, id)?;

    open_editor(&file)?;

    // Refresh the `updated` timestamp after editing.
    let mut doc = read_doc(&file)?;
    doc.set("updated", &util::utc_now());
    write_doc(&file, &doc)?;

    let rel = format!("issues/{}.md", issue_id(id));
    commit(&s.path, &[&rel], &format!("issues: update {id}"))?;
    println!("Updated {id}");

    if sync {
        sync_worktree(&s)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// close / reopen
// ---------------------------------------------------------------------------

pub fn set_status(id: &str, status: &str, sync: bool) -> Result<()> {
    let s = config::resolve()?;
    ensure_worktree(&s)?;
    let file = existing_issue(&s, id)?;

    let mut doc = read_doc(&file)?;
    doc.set("status", status);
    doc.set("updated", &util::utc_now());
    write_doc(&file, &doc)?;

    let rel = format!("issues/{}.md", issue_id(id));
    commit(&s.path, &[&rel], &format!("issues: {status} {id}"))?;
    println!("Marked {id} as {status}");

    if sync {
        sync_worktree(&s)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// sync / path / status
// ---------------------------------------------------------------------------

fn sync_worktree(s: &Settings) -> Result<()> {
    let remote = s
        .remote
        .as_deref()
        .context("no remote configured for this repository; nothing to sync")?;

    // Fetch may fail if the branch doesn't exist on the remote yet — that's fine,
    // we'll create it on push. Captured to avoid a noisy fatal on first sync.
    let fetched = git::output(&["-C", &s.path, "fetch", remote, &s.branch])
        .map(|o| o.status.success())
        .unwrap_or(false);
    if fetched && !git::run(&["-C", &s.path, "merge", "--no-edit", "FETCH_HEAD"])? {
        bail!(
            "merge conflict while syncing — resolve it inside the issues worktree, \
             then re-run `git issues sync`"
        );
    }
    if !git::run(&["-C", &s.path, "push", "-u", remote, &s.branch])? {
        bail!("push failed");
    }
    println!("Synced with {remote}/{}", s.branch);
    Ok(())
}

pub fn sync() -> Result<()> {
    let s = config::resolve()?;
    ensure_worktree(&s)?;
    sync_worktree(&s)
}

pub fn path() -> Result<()> {
    let s = config::resolve()?;
    println!("{}", s.path);
    Ok(())
}

pub fn status() -> Result<()> {
    let s = config::resolve()?;
    ensure_worktree(&s)?;
    // `-sb` prints the branch line with ahead/behind vs. the upstream.
    git::run(&["-C", &s.path, "status", "-sb"])?;
    Ok(())
}
