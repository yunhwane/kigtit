# How it's built

[한국어](design.ko.md)

Why the choices are what they are. For how to use it, see the
[README](../README.md).

## Who this is for

Existing Git GUIs (Sourcetree, GitKraken, GitHub Desktop) are tools for
*developers who know Git and find the CLI tedious*. Someone building with AI has
different problems:

1. **The AI broke something that used to work and they don't know where to go
   back to** — by a wide margin the biggest one
2. **They don't know what changed** — they can't read a diff. Thirty changed
   files is terrifying
3. **They commit an API key and leak it** — the point where real money is lost

Kigtit solves only these three. Everything else it deliberately doesn't do.

## Design principles

| | |
|---|---|
| **There is no commit button** | A few quiet seconds after a file changes, it saves. Nobody has to learn that saving is a thing you must do. |
| **Undo is recorded too** | `reset --hard` is never used. An undo becomes a new save point. No path inside the app destroys work permanently. |
| **Sentences before diffs** | Every explanation is plain language, not code. Code unfolds only when asked. |
| **"Does the app start" is pinned to the timeline** | Every save actually runs the app. On failure it shows the reason. |
| **Conflicts never demand hunk merging** | `<<<<<<< HEAD` is never shown. Both sides are explained, and the choice is per file. |
| **State is never colour alone** | Starts `●`, broken `■`, warning `▲`. Readable when colour blind or in black and white. |
| **Only block what's dangerous** | Secret keys, large files, dependency folders. Nothing else interrupts. |

## Vocabulary

Nothing in the interface uses Git's words.

| Git | What the UI says | Literally | Why |
|---|---|---|---|
| commit | 세이브 포인트 | save point | A concept already learned from games |
| revert / reset | 여기로 되돌리기 | go back to here | The user doesn't need to know the difference |
| branch | 다른 방법으로 해보기 | try it another way | Named for its purpose |
| diff | 무엇이 바뀌었나 | what changed | A question reads better |
| push | GitHub에 백업 | back up to GitHub | Everyone knows what a backup is |
| pull | GitHub와 맞추기 | line up with GitHub | |
| merge conflict | 선택이 필요해요 | a choice is needed | "Conflict" frightens; "choice" prompts action |
| .gitignore | 백업에서 빼두기 | leave out of the backup | Never exposes the filename |

## It takes no API keys

Plain-language summaries **borrow an AI CLI that's already installed.** The target
user builds with AI, so `claude` or `codex` is already there and signed in.
Backup borrows an already-authenticated `gh` for the same reason.

- No settings screen
- No separate bill
- No surface for a key to leak from

Priority is `claude` → `codex` → a rule-based fallback. With none of them the
feature doesn't die; it says as much as a file list allows.

### Summaries arrive later

One summary takes about 8 seconds. Editing a commit message would change the
hash, so it's attached afterwards as JSON in `refs/notes/kigtit`. **The hash
stays the same.**

There is no queue file. "Save points without a summary" *is* the queue, and it
already lives in Git. A queue file could only get corrupted or drift out of sync
with reality. On startup the watcher scans the most recent 40 and fills in up to 8.

### It doesn't rely on PATH

An app launched from Finder or the Dock does not inherit the login shell's PATH.
launchd gives it roughly `/usr/bin:/bin:/usr/sbin:/sbin`, so `claude` (usually in
`~/.local/bin`) and `pnpm` (usually in `/opt/homebrew/bin`) **look like they don't
exist.** Things that work from a terminal die silently when the icon is
double-clicked — the hardest kind of bug to find.

So [`crates/core/src/tools.rs`](../crates/core/src/tools.rs) checks PATH first
and, failing that, searches the places tools actually get installed, then runs
them by absolute path. Spawning a login shell to harvest PATH is the other
option, but a weird profile would hang app startup. A static candidate list is
safer and faster.

## How it knows whether the app starts

It works out what kind of project this is and actually runs something.

| Project | How it checks |
|---|---|
| `package.json` with a `build` script | `<runner> run build` — catches type errors, missing files, syntax errors |
| `package.json` with `tsconfig.json` | `tsc --noEmit` |
| `Cargo.toml` | `cargo check` |
| `pyproject.toml` or `.py` files | `python3 -m compileall` |

The verdict has **three branches**, not two: success, failure, and *can't tell*.
Recording a failure when there was no way to check would send the user back to
the wrong point. If `node_modules` is missing, nothing you run tells you whether
the failure is real, so no verdict is recorded.

A build can take 30 seconds, so spawning one per save would pile them up. There
is a single checker thread, and a backlog collapses to **only the newest** request.
If a new save point appears mid-check the result is thrown away — pinning `■` to
the wrong point is worse than being slow. The verdict converges, late.

## A choice is needed (conflicts)

Conflicts are where non-developers give up. Most people make a copy of the folder.

**Hunk-level merging is never demanded.** Showing `<<<<<<< HEAD` to someone who
never learned to code helps with nothing. The choice is per file: mine or theirs.
Files that don't overlap merge normally, so the choice applies only to files that
genuinely collided.

**Instead, an AI explains what each side was trying to do.** This is the only
information that makes the choice possible.

```
File: config.js

[On my computer]
The cafe's hours changed from closing at 10pm to closing at 6pm, so it now
closes earlier. Information about being closed on Sundays was added.

[On GitHub]
The cafe's name became '내 카페 ☕', and hours were extended from 7am to 11pm.
A notice was added saying early-morning delivery has started.
```

**A half-merged state is never created.** The merge is computed in memory, and if
there's a conflict the working folder is **left untouched** and only the list is
returned. Conflict markers never leak into files, and the repository never sits in
`MERGING`. Quit halfway and nothing is lost.

## The gate before backup

`kigtit check` only looks at changes that haven't been stored yet. But **pushing
publishes history** — a key committed earlier leaks as-is. So backup scans **every
tracked file** and refuses to upload if it finds a single secret.

```
$ kigtit check
  ●  위험한 파일이 없어요.          ← only looks at unstored changes, so it misses

$ kigtit backup
  ▲ 백업을 멈췄어요
     config.js 1번째 줄에 OpenAI 키처럼 보이는 값이 있어요...
     sk-proj-••••••B4nM
```

What it looks for: OpenAI, Anthropic, AWS, Google, GitHub, Slack and Stripe keys,
private key files, and settings shaped like `api_key = "..."`.

Fresh projects get their first branch from `init.defaultBranch`, falling back to
`main`. git2's own default is still `master`, which would make branch names
disagree at backup time.

## Layout

```
crates/core/     All the logic. The CLI and the desktop app share it.
  repo.rs        Opening a folder / automatic .gitignore
  save.rs        Creating save points
  restore.rs     Undo — the way that loses nothing
  timeline.rs    Reading the timeline
  secrets.rs     Scanning for secret keys and large files
  ai.rs          Plain-language summaries via a local AI CLI
  backup.rs      GitHub backup — borrows gh
  sync.rs        Reconciling, and "a choice is needed"
  health.rs      Actually running the app to see if it starts
  tools.rs       Finding external tools without relying on PATH
  notes.rs       Information attached later (refs/notes/kigtit)
  watch.rs       The autosave daemon
crates/cli/      The kigtit binary
src-tauri/       Desktop app backend. A thin layer over core.
src/             React screens
design/          Screen designs (screens.html) + the icon generator (icon.py)
demo/            GIFs, screenshots, and the scripts that produce them
docs/            This
```

The app and the CLI share `crates/core` verbatim, so behaviour can't diverge.

`git2::Repository` can't be moved across threads, so it isn't held in state. Every
command reopens by path, which costs milliseconds.
