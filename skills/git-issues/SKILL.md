---
name: git-issues
description: Use whenever creating, editing, listing, closing, or syncing issues — including bare requests like "issue立てて" / "create an issue" / "open an issue". In this repo, issues are tracked with the git-issues CLI (git issues new/edit/list/show/close/reopen/sync), NOT with `gh issue`. Especially in a non-interactive/agent environment, or when `git issues new` / `git issues edit` hangs or blocks waiting for an editor.
---

# git-issues

## Overview

`git-issues` tracks issues as Markdown files on a dedicated orphan branch (default `meta/issues`), checked out in a **separate worktree** so they sync via `git push`/`pull` like anything else. Each issue is one file at `<worktree>/issues/<id>.md` with a small `key: value` front-matter block (`id`, `title`, `status`, `labels`, `created`, `updated`) followed by a Markdown body.

**The one non-obvious thing:** `new` and `edit` launch your interactive `$EDITOR` to fill in the body. In an agent/non-TTY environment they will **hang or fail**. There is no `--body` flag. Drive them non-interactively (below).

## The trap — read this first

```bash
git issues new "Some title"      # ❌ opens $EDITOR → hangs an agent forever
git issues edit some-id          # ❌ same
```

Set `GIT_EDITOR` to a non-interactive command so the editor step is a no-op, then put the body into the file yourself. `GIT_EDITOR` wins over `EDITOR`/`VISUAL` for this tool.

## Creating an issue (agent-safe recipe)

```bash
# 0. First time in a repo only: set up the worktree.
git issues init                          # or: init --branch <b> --path <p>

# 1. Scaffold the issue with an empty body (no editor opens).
GIT_EDITOR=true git issues new "Search returns stale results"
#   → prints:  Created search-returns-stale-results-x49r   (id = slug + random4)

# 2. Find the file and write the real body with your normal file tools.
WT=$(git issues path)                    # absolute worktree path
#   edit  $WT/issues/<id>.md  — append your body AFTER the closing `---` fence.
#   To add labels, set the `labels:` line in the front-matter (comma/space separated).

# 3. Commit your edits (refreshes `updated`, no editor opens).
GIT_EDITOR=true git issues edit <id>
```

Prefer editing the file with the Write/Edit tools over shell heredocs. Keep the front-matter fences (`---`) intact; only the body and the `labels:` value are yours to change.

Any status-changing command commits pending file edits too. If your next step is `close`/`reopen`, you can skip step 3 — that command will commit your hand-edits. Use the explicit `edit` only when you want the change recorded on its own.

## Quick reference — safe, non-interactive commands

| Command | Purpose |
|---|---|
| `git issues list` | List all issues (id / status / title) |
| `git issues list --status open` (or `closed`) | Filter by status |
| `git issues list --label bug` | Filter by label |
| `git issues show <id>` | Print one issue's raw file |
| `git issues close <id>` / `reopen <id>` | Flip status (commits) |
| `git issues status` | Ahead/behind of the issues branch |
| `git issues path` | Print the worktree path |
| `git issues sync` | fetch + merge + push the issues branch |

- **`id`**: the `<slug>-<rand4>` string; the trailing `.md` is optional in commands.
- **`--sync`** on `new`/`edit`/`close`/`reopen` pushes right after the change (needs a remote).
- `list`, `show`, `status`, `path` never touch an editor — use them freely.

## Common mistakes

- Running `new`/`edit` without `GIT_EDITOR=true` → hangs waiting on `$EDITOR`. Always set it.
- Trying to pass the body on the CLI → there is no `--body` flag; the body lives in the file.
- Editing an issue by hand and forgetting to commit → run `GIT_EDITOR=true git issues edit <id>` (or just `git -C "$(git issues path)" add . && git -C ... commit`) so it's recorded and `updated` is refreshed.
- Operating before `git issues init` → commands error with "issues worktree not found". Run `init` (or `git issues status` to check) first.
- Corrupting the front-matter: it is a tiny `key: value` format fenced by `---`, **not** full YAML — keep both fences and one `key: value` per line.
