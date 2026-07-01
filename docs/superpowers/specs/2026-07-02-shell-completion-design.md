# シェル補完設計（issue ID の動的補完）

- 日付: 2026-07-02
- 対象: `git-issues` CLI
- ステータス: 承認済み（実装前）

## 目的

`git issues` の以下4コマンドで、issue ID 引数を **動的にタブ補完** できるようにする。

- `git issues show <TAB>`
- `git issues edit <TAB>`
- `git issues close <TAB>`
- `git issues reopen <TAB>`

対象シェルは **zsh・bash・fish**。呼び出しは `git issues …`（git のサブコマンド形式）を前提とする。

補足: `sync` は issue ID を引数に取らない（fetch/merge/push のみ）ため、ID 補完の対象外。サブコマンド名としての補完のみ受ける。

## 非目標（YAGNI）

- タイトルや説明文を候補に併記する（MVP は ID のみ。将来拡張として保留）。
- 補完スクリプトの自動インストール（本ツールは出力のみ。設置は手動手順を案内）。
- `clap_complete` による静的補完や unstable な動的補完エンジンの採用。
- フラグ値（`--status` 等）の値補完（サブコマンド名とフラグ名の補完までに留める）。

## 方針

`git issues` は git のサブコマンド（`git-issues` バイナリ）なので、補完は各シェルの
「git サブコマンド拡張ポイント」に乗せる。ID 候補は補完実行時に本バイナリへ問い合わせて
動的に得る。

- bash: git 補完が `_git_<subcmd>` を自動で呼ぶ → `_git_issues` 関数を定義。
- zsh: `_git` が `_git-<subcmd>` に委譲する → `_git-issues` 関数を定義。
- fish: `__fish_seen_subcommand_from issues` 条件で `complete -c git` を定義。

## コンポーネント

### 1. `git issues completions <shell>`（公開サブコマンド）

- 指定シェル向けの補完スクリプトを **標準出力に出すだけ**。
- `<shell>` は clap の `ValueEnum`：`zsh` / `bash` / `fish`。
- スクリプト本体は `src/completions/{bash,zsh,fish}` に平文で置き、`include_str!` で埋め込む
  （`SKILL.md` を `include_str!` する既存方式に倣う）。

### 2. `git issues complete-ids`（隠しサブコマンド、`hide = true`）

補完スクリプトから呼ばれる内部ヘルパー。**絶対に失敗・ハングしない**こと。

出力契約:

- 設定解決に失敗、または worktree 未初期化なら **何も出さず正常終了（exit 0）**。
- `<worktree>/issues/*.md` を走査し、各 issue の ID を1行1件で出力。
  - ID は frontmatter の `id`、無ければファイル名 stem。
- 出力は **ソート済み**。
- エラーは stdout に出さない（候補に混入するため）。IO エラーは該当分をスキップ。
- 前方一致フィルタはしない（絞り込みはシェル側が担当）。
- エディタ起動や worktree への書き込みなど、副作用のある処理は一切行わない。

### 3. シェル別スクリプトの挙動

いずれも第1位置引数はサブコマンド名を補完し、`show/edit/close/reopen` の ID 位置引数で
`git issues complete-ids` を呼んで候補化する。フラグ名（`--sync` 等）も補完する。

- **bash** (`_git_issues`): git-completion.bash が提供する `$cur`/`$prev`/`$words`/`$cword`
  を用い、対象コマンドの ID 位置では
  `COMPREPLY=($(compgen -W "$(git issues complete-ids)" -- "$cur"))`。
- **zsh** (`_git-issues`): `_arguments`/`_describe` でサブコマンドを補完し、対象コマンドでは
  `compadd -- $(git issues complete-ids)`。
- **fish**: 例
  `complete -c git -n '__fish_seen_subcommand_from issues; and __fish_seen_subcommand_from show edit close reopen' -f -a '(git issues complete-ids)'`
  に加え、サブコマンド名の補完も定義。

## コード構成

- 新規 `src/completions.rs`
  - `pub enum Shell { Bash, Zsh, Fish }`（`clap::ValueEnum` 派生）
  - `pub fn print(shell: Shell) -> anyhow::Result<()>` — 埋め込みスクリプトを出力。
- 新規 `src/completions/{bash,zsh,fish}` — 補完スクリプト本体。
- `src/cmd.rs`
  - ID 列挙のコアを `collect_issue_ids(s: &Settings) -> Vec<String>` に切り出す
    （純粋関数寄りでテスト可能に）。既存 `list` の走査ロジックも可能なら本関数に寄せる。
  - `pub fn complete_ids() -> anyhow::Result<()>` — `collect_issue_ids` を呼び、
    未初期化・エラーは握りつぶして **常に `Ok`**。1行1件で標準出力へ。
- `src/main.rs`
  - `Command` に `Completions { shell: completions::Shell }` と 隠し `CompleteIds` を追加。
  - `CompleteIds` のディスパッチは失敗させない（`complete_ids()` は常に `Ok`）。

## インストール手順（ドキュメント／`completions` のヘルプに記載）

- zsh: `git issues completions zsh > <fpath上のディレクトリ>/_git-issues`（＋ `compinit`）
- bash: `git issues completions bash > ~/.git-issues-completion.bash` を、git 補完読み込みの
  後に `source ~/.git-issues-completion.bash`
- fish: `git issues completions fish > ~/.config/fish/completions/git-issues.fish`

## テスト方針

- `collect_issue_ids`
  - 一時 worktree に issue ファイルを複数置き、**ソート済み ID** が返ること。
  - worktree 未初期化なら **空** が返ること。
  - frontmatter に `id` が無い場合はファイル名 stem を用いること。
- `completions <shell>`
  - 各シェルで出力が空でなく、コールバック文字列 `git issues complete-ids` を含むこと。
- 手動確認: zsh/bash/fish それぞれで `git issues show <TAB>` が既存 issue ID を提示すること。

## 想定エッジケース

- worktree 未初期化 → 候補ゼロ、エラー出力なし、補完は静かに何も返さない。
- issue が0件 → 候補ゼロ。
- `id` に空白等の特殊文字が入り得るか: 現状 `util::make_id` はスラッグ化されており空白を含まない
  想定。将来特殊文字が入る場合は各シェルでのクォートを再検討（本 MVP では対象外）。
