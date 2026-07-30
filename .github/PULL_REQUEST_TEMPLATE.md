## What this changes

<!-- One or two sentences. What's different after this lands? -->

## Why

<!-- If it isn't obvious from the above. Link an issue if there is one. -->

## How you tested it

<!-- Be concrete. "Ran `kigtit health` on a Python file with an unclosed paren
     and it reported the reason" is enough. "Should work" is not.

     If something is untested or unfinished, say so here. Partial work is
     welcome when it's labelled. -->

## Checklist

- [ ] `cargo test -p kigtit-core` passes
- [ ] `cargo clippy` is clean
- [ ] `pnpm build` passes (runs `tsc --noEmit`)
- [ ] Added a `CHANGELOG.md` entry under `## [Unreleased]`
- [ ] Regenerated affected screenshots/GIFs if a screen changed
      (`cd demo && ./record.sh`, `node demo/shots.mjs`)

## Design principles

<!-- CONTRIBUTING.md lists seven constraints the codebase is built around.
     If this change bends one of them, argue for it here. Otherwise delete. -->
