# Using the CLI

[한국어](cli.ko.md)

Everything the app does works from a terminal. Building with AI happens in a
terminal, so `kigtit` was a CLI before it was an app.

The app is covered in the [README](../README.md).

**Output is in Korean.** Kigtit was built for Korean-speaking non-developers, so
every message it prints is Korean. This page explains what each command does in
English and shows the real output as it appears.

## Install

```sh
cargo install --path crates/cli
```

## At a glance

```sh
kigtit                    # timeline for the current folder
kigtit open               # open this folder in the app window
kigtit watch              # turn on autosave
kigtit save "a note"      # save on purpose
kigtit show <id> --code   # what changed
kigtit undo               # back to before the last save point
kigtit back <id>          # back to a specific point
kigtit health             # actually run the app and record whether it starts
kigtit mark ok|broken     # override that verdict by hand
kigtit check --fix        # scan for secret keys and large files
kigtit backup             # back up to GitHub (private by default)
kigtit sync               # pull changes from GitHub and reconcile
kigtit summarize          # fill in missing summaries
```

Add `-C <folder>` to target a different folder.

## `kigtit` — the timeline

Run it with no arguments and you get the timeline for the current folder.

```
$ kigtit

  ◆  지금          아직 저장되지 않은 변경 2개
  │               app/page.tsx, lib/menu.ts
  │               `kigtit save`로 담을 수 있어요
  │
  ■  오후 2:47     장바구니 표시 추가                    801b8bf
  │               장바구니에 담긴 상품을 보여주는 화면이 새로 만들어졌습니다.
  │               첫 번째 상품의 이름 다음에 나머지 개수를 표시해서 사용자가
  │               장바구니 내용을 한눈에 볼 수 있게 됩니다.
  │               여기서 앱이 안 켜졌어요  → kigtit back 2715681  (주문 버튼 디자인 변경)
  │               *** Error compiling './cart.py'...
  │
  ●  오후 2:31     주문 버튼 디자인 변경                  2715681
  │               주문하기 버튼이 초록색에서 파란색으로 바뀌었습니다. 버튼의
  │               모서리가 더 둥글어지고 그림자 효과가 추가되었습니다.
  │
  ○  오후 1:20     프로젝트 시작                         da8d2b9
```

How to read it:

| Mark | Meaning |
|---|---|
| `●` green | The app started fine at this point |
| `■` red | **The app stopped starting here** — the reason and where to go back to follow on the next lines |
| `○` grey | Not checked yet |
| `◆` plum | Changes not saved yet |

Shape and colour are both used, so it reads correctly if you're colour blind or
piping to a file.

```sh
kigtit list --limit 30    # show more
```

## `kigtit watch` — autosave

```sh
kigtit watch              # save after 3 quiet seconds
kigtit watch --idle 10    # make it 10 seconds
```

`Ctrl+C` to stop. Killing it is fine — on the next start it picks up the
summaries it missed.

![Autosave](../demo/watch.gif)

Each save does three things: it stores the change, runs the app to see whether it
still starts, and asks the local AI CLI to describe what happened.

## `kigtit save` — save on purpose

```sh
kigtit save                    # the AI writes the title
kigtit save "add menu photos"  # you write the title
kigtit save --no-summary       # don't wait for the AI
```

```
$ kigtit save
  ●  저장했어요 2715681  자동 저장 · 파일 1개
     Claude Code로 요약하는 중…
     주문 버튼 디자인 변경
     주문하기 버튼이 초록색에서 파란색으로 바뀌었습니다. 버튼의 모서리가
     더 둥글어지고 크기가 커졌으며, 그림자 효과가 추가되었습니다.
```

## `kigtit show` — what changed

```sh
kigtit show                  # the most recent one
kigtit show 801b8bf          # a specific point
kigtit show 801b8bf --code   # unfold the code too
```

The plain-language explanation comes first. Code only appears with `--code`.

## `kigtit undo` / `kigtit back` — going back

```sh
kigtit undo               # back to before the last save point
kigtit back 2715681       # back to a specific point
```

```
$ kigtit back 2715681
  ●  주문 버튼 디자인 변경 시점으로 되돌렸어요 2715681
     되돌리기 직전 상태는 c871216에 담아뒀어요.
     되돌린 것도 되돌릴 수 있어요 → kigtit undo
```

![Undo](../demo/undo.gif)

Unsaved work in progress is fine. It gets stored before the undo happens, so
nothing is lost.

## `kigtit health` — does the app start?

This runs on its own every time something is saved. Use the command when you
want to check again right now.

```
$ kigtit health
  확인하는 중… (문법 검사)
  ■  여기서 앱이 안 켜졌어요  (문법 검사)

     *** Error compiling './cart.py'...
       File "./cart.py", line 4
         return sum(i["price"] for i in items
                   ^
     SyntaxError: '(' was never closed

  마지막으로 잘 켜졌던 시점으로 돌아가려면 `kigtit list`를 보세요.
```

If the verdict is wrong you can override it.

```sh
kigtit mark ok
kigtit mark broken
kigtit mark unknown 801b8bf
```

## `kigtit check` — risky files

```sh
kigtit check          # scan only
kigtit check --fix    # exclude large files from backup right away
```

```
$ kigtit check
  ■  assets/promo.mov 파일이 7MB예요. 백업에 넣으면 나중에 느려집니다.
     추천: 백업에서 빼두기
  ▲  lib/openai.ts 2번째 줄에 OpenAI 키처럼 보이는 값이 있어요.
     이대로 올리면 남이 가져다 쓸 수 있습니다.
     sk-proj-••••••B4nM
     추천: 키를 .env 파일로 옮기고 백업에서 빼두기
```

The full value is never printed.

## `kigtit backup` — back up to GitHub

```sh
kigtit backup             # private (default)
kigtit backup --public    # visible to everyone
kigtit backup --status    # status only, don't upload
```

```
$ kigtit backup
  ●  yunhwane 계정으로 올릴 수 있어요
     아직 연결된 곳이 없어요. 새로 만들어 드릴게요.
     백업 안 된 세이브 포인트 2개

  나만 볼 수 있게 올립니다.
  올리는 중…
  ●  백업했어요 — 세이브 포인트 2개
     https://github.com/yunhwane/my-project.git
     저장소를 새로 만들었어요.
```

If there's a single key anywhere in the tracked files, **it does not upload.**

![Blocked key leak](../demo/secret.gif)

Note the contrast: `kigtit check` only looks at changes that haven't been stored
yet, so it says everything is clean. `kigtit backup` scans **every tracked file**,
because pushing publishes history — a key committed earlier would leak.

## `kigtit sync` — reconcile with GitHub

Pulls in work done on another computer.

```sh
kigtit sync                 # pull and reconcile
kigtit sync --keep mine     # overlapping files: keep mine
kigtit sync --keep theirs   # overlapping files: keep GitHub's
```

When files overlap it stops and asks. **Your working folder is untouched at this
point.**

```
$ kigtit sync
  ▲ 선택이 필요해요 — 아래 파일을 양쪽에서 같이 고쳤어요.
  작업 폴더는 아직 그대로입니다. 아무것도 잃지 않았어요.

    ▲ config.js

  어느 쪽을 남길지 고르세요:
    kigtit sync --keep mine    내 컴퓨터에서 한 것을 남긴다
    kigtit sync --keep theirs  GitHub에 있던 것을 남긴다
```

![A choice is needed](../demo/sync.gif)

Files that don't overlap merge normally, so the choice only applies to files that
genuinely collided.

## `kigtit open` — open the app window

```sh
kigtit open
```

For when you've been working in the terminal and want to look at it. The app
opens straight into that folder.

## `kigtit summarize` — fill in summaries

```sh
kigtit summarize
kigtit summarize --limit 50
```

Fills in summaries missing because you used `--no-summary` or because the app was
killed mid-flight.

---

## Working on Kigtit

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full loop. The short version:

```sh
cargo test -p kigtit-core   # unit tests
cargo clippy                # lint
cargo tauri dev             # the app in dev mode
pnpm build                  # frontend only (includes tsc)
```

`cargo run` on the app gives you a blank window — in debug builds Tauri looks at
`devUrl` (localhost:1420). Use `cargo tauri dev`, or build release so the
embedded frontend is used.

DMG packaging needs Finder permissions, so it's off by default. Add `"dmg"` to
`bundle.targets` in `src-tauri/tauri.conf.json` if you want it.

### Regenerating the screenshots and GIFs

Neither is captured by hand. The scripts live in the repo so anyone can
reproduce the same output.

```sh
brew install vhs
brew install --cask font-d2coding   # Korean monospace font

cd demo && ./record.sh              # terminal GIFs
pnpm build && node demo/shots.mjs   # app screenshots
```

`demo/shots.mjs` renders the **actual screen code** from `src/` — not a mockup.
Only the backend is replaced with sample data. It runs in a headless browser
rather than the Tauri window, because capturing the real window needs macOS
Screen Recording permission.
