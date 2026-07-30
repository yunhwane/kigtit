# Changelog

Notable changes to Kigtit. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Maintained by hand. Add your entry under `## [Unreleased]` in your pull request.

## [Unreleased]

Nothing yet.

## [0.1.0] — not released yet

The first shape of the whole thing. Nothing has been tagged; this documents what
exists on `main`.

### Added

**Save points, without the vocabulary**

- Opening a folder prepares it with no questions asked, and adds `node_modules/`,
  `.env` and build output to `.gitignore` automatically.
- Autosave: a few quiet seconds after a file changes, the state is stored. While
  an AI is still writing files it keeps waiting, so one thing you asked for
  becomes one save point.
- Undo is recorded as a save point of its own, so an undo can be undone. There is
  no path in the app that destroys work permanently; the state right before an
  undo is stored separately.
- Timeline output uses shape *and* colour for every status, so it reads correctly
  when colour blind or in black and white.

**Plain-language summaries, no API key**

- Diffs are summarized in Korean by an AI CLI the user already has installed
  (`claude`, then `codex`, then a rule-based fallback). No key is ever requested.
- Summaries attach afterwards as JSON in `refs/notes/kigtit`, so commit hashes
  never change.
- Missed summaries resume on the next start. There is no queue file — save points
  without a summary *are* the queue, and that already lives in Git.

**Does the app start**

- Every save actually runs the project: `<runner> run build`, `tsc --noEmit`,
  `cargo check`, or `python3 -m compileall`, whichever fits.
- The verdict has three branches — starts, broken, or *can't tell*. A project with
  no way to check gets no verdict, because a wrong failure sends someone back to
  the wrong point.
- Failures show the reason, and the timeline points at the last save point that
  did start.
- One checker thread; a backlog collapses to only the newest request, and results
  are discarded if a new save point landed mid-check.

**Not leaking keys**

- Scans for OpenAI, Anthropic, AWS, Google, GitHub, Slack and Stripe keys, private
  key files, and settings shaped like `api_key = "..."`. Values are always masked.
- Large files (>5 MB) are flagged and can be excluded in one step.
- Backup scans **every tracked file**, not just unstored changes, because pushing
  publishes history.

**GitHub**

- Backup borrows an already-authenticated `gh`; no token is ever requested.
  Private by default, public only on an explicit opt-in.
- Sync pulls remote changes and merges cleanly when files don't overlap.
- Conflicts never show `<<<<<<< HEAD`. Both sides are explained in plain language
  by the AI CLI, and the choice is per file.
- A half-merged state is never created: the merge is computed in memory, and on
  conflict the working folder is left untouched until every choice is made.

**Surfaces**

- `kigtit` CLI with 13 commands, including `kigtit open` to raise the app window
  on the current folder.
- macOS desktop app (Tauri + React) sharing `crates/core` with the CLI, so
  behaviour can't diverge.
- Reproducible demo assets: `demo/record.sh` (terminal GIFs via vhs) and
  `demo/shots.mjs` (app screenshots by rendering the real screen code).

### Fixed

Found while building the demos and documentation:

- External tools were resolved through `PATH` only, so `claude` and `pnpm` looked
  absent when the app was launched from Finder — plain-language summaries died
  silently for anyone who double-clicked the icon. `crates/core/src/tools.rs` now
  searches real install locations and runs tools by absolute path.
- New repositories were created on `master`, ignoring `init.defaultBranch`, which
  made branch names disagree at backup time.
- Timeline entries created within the same second came out in arbitrary order;
  the walk now sorts topologically as well as by time.
- The file list in the details panel was a flex child with no shrink control, so
  files were silently clipped — "3 files" could show one.
- Error output was wrapped, which moved the `^` caret away from the character it
  points at.
- Timeline nodes distinguished state by colour alone, and in the dark theme the
  failure colour was nearly identical to the accent. Broken points are now a
  square, matching the `■` used elsewhere.
- The account name parser read any word following "account", so an unexpected `gh`
  output could be shown to the user as their username.

### Known gaps

- No signed installer, so a non-developer cannot install this alone.
- Interface is Korean only; no localization layer exists.
- macOS only.
- Keys deleted from history aren't detected — only what's in the files now.
- Conflict choices are per file; "keep a bit of both" isn't possible.
- Multiple people working together isn't handled.
- No non-developer has used it yet.

[Unreleased]: https://github.com/yunhwane/kigtit/compare/main...HEAD
[0.1.0]: https://github.com/yunhwane/kigtit/tree/main
