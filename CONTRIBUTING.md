# Contributing

Thanks for looking. Kigtit is small and opinionated, and the fastest way to help
is probably not code — see [What helps most](#what-helps-most).

## What helps most

**Watch a non-developer use it.** Nobody has, yet. Every problem the README
claims to solve is still a hypothesis. If you sit next to someone who builds with
AI and can't read a diff, and you write down where they got stuck, that is worth
more than any pull request right now.

**Report where the wording fails.** The whole product is a bet that plain
sentences beat Git vocabulary. If a message confused someone, that's a bug. Open
an issue with the exact text.

**Try it on a project shape we don't handle.** The health check knows about
Node, Rust and Python. If your project is Go, Ruby, Swift, or a plain static
site, `kigtit health` will say it can't tell. Tell us what should have run.

## Ground rules for changes

These are the constraints the codebase is built around. A change that breaks one
of them needs a strong argument in the pull request.

1. **No path may destroy work permanently.** `reset --hard` and force-checkout
   over unsaved changes are out. Store first, then move.
2. **No Git vocabulary in the interface.** Not in labels, not in errors, not in
   toasts. See the vocabulary table in [docs/design.md](docs/design.md).
3. **Never state a verdict you can't support.** If the health check has no way to
   run, it records "can't tell", not "broken". A wrong `■` sends someone back to
   the wrong point.
4. **State is never colour alone.** Every status carries a shape too.
5. **Interrupt only for real danger.** Secret keys, large files, dependency
   folders. Nothing else gets a modal.
6. **Take no API keys.** Borrow tools the user has already signed into. If a
   feature needs a key, it probably shouldn't ship.
7. **Logic goes in `crates/core`.** The CLI and the app must not diverge. Both are
   thin layers.

## Setup

Requires macOS, [Rust](https://rustup.rs), and [Node](https://nodejs.org) with
[pnpm](https://pnpm.io).

```sh
git clone https://github.com/yunhwane/kigtit
cd kigtit
pnpm install
```

## The loop

```sh
cargo test -p kigtit-core   # unit tests
cargo clippy                # lint — keep it clean
cargo build                 # the whole workspace

cargo install --path crates/cli   # install the kigtit command
cargo tauri dev                   # run the app against a dev server
pnpm build                        # frontend only (runs tsc first)
```

`cargo run -p kigtit-app` gives you a blank window. In debug builds Tauri loads
`devUrl` (localhost:1420), so the frontend has to be running. Use
`cargo tauri dev`, or `cargo tauri build` for a release bundle with the frontend
embedded.

### Testing by hand

The features that matter are hard to unit test, so exercise them against a
throwaway project:

```sh
mkdir /tmp/probe && cd /tmp/probe
printf '[project]\nname="p"\nversion="0.1.0"\n' > pyproject.toml
echo 'X = 1' > main.py

kigtit save "first"        # creates the repo, no questions asked
echo 'def broken(' > bad.py
kigtit health              # should say it stopped starting, and why
kigtit undo                # should go back and lose nothing
kigtit health              # should be clean again
```

For conflicts you need two clones and a bare remote — `demo/record.sh` sets that
up already, so read `seed_conflict` there.

## Style

- **Comments explain why, not what.** The code says what. If a line looks odd,
  the comment should say what would break otherwise.
- **User-facing strings are Korean.** Identifiers and comments are English.
  There's no localization layer yet, so strings are inline.
- **Rust**: `cargo clippy` clean. Errors that reach a user are sentences someone
  who doesn't code could act on.
- **TypeScript**: `pnpm build` runs `tsc --noEmit`; keep it passing. No `any`.
- **CSS**: tokens live at the top of `src/styles.css`. Use them; don't hardcode
  colours. Both light and dark have to work.

## Screenshots and GIFs

Neither is captured by hand — the scripts are in the repo so the output is
reproducible.

```sh
brew install vhs
brew install --cask font-d2coding   # Korean monospace; without it Korean spaces out

cd demo && ./record.sh              # terminal GIFs
pnpm build && node demo/shots.mjs   # app screenshots
```

If you change a screen, regenerate the affected images in the same pull request.

`demo/record.sh` reseeds its sample project before each recording, because some
tapes actually perform an undo or a merge. Skipping that leaves traces of the
previous recording in the next GIF.

## Pull requests

- One concern per pull request.
- Say what you tested, and how. "Ran `kigtit health` on a broken Python file" is
  enough; "should work" is not.
- If tests fail or something is unfinished, say so in the description. Partial
  work is welcome when it's labelled.
- If your change touches a design principle above, argue for it explicitly.

## Commit messages

No enforced convention. Write a subject line that says what changed, and a body
that says why if it isn't obvious. Korean or English are both fine.

`CHANGELOG.md` is maintained by hand — add your entry under `## [Unreleased]`.

## Releases

Maintainers only. See [CHANGELOG.md](CHANGELOG.md) for what's shipped.

```sh
GITHUB_TOKEN=$(gh auth token) pnpm release
GITHUB_TOKEN=$(gh auth token) pnpm release --dry-run
```

**The token matters.** Without `GITHUB_TOKEN`, release-it falls back to opening a
web form and the `releaseNotes` command is never run — you get the literal string
`node scripts/release-notes.mjs ${version}` as your release body. With a token it
uses the API and the notes are generated properly.

`release-it` bumps `package.json`, then `scripts/sync-version.mjs` writes the same
version into the Cargo workspace and `src-tauri/tauri.conf.json` so all three stay
in step. `scripts/release-notes.mjs` pulls the matching section out of
`CHANGELOG.md`, so the changelog is the only place release notes are written.

**`--dry-run` still edits `package.json`.** release-it runs `npm version` for real
even in a dry run. Follow a dry run with `git checkout -- package.json`.

The release is created as a **draft** on purpose — `Kigtit.app` has to be attached
by hand after `cargo tauri build`, since CI can't sign it.

## Code of conduct

By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
