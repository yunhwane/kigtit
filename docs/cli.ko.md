# 터미널에서 쓰기

앱으로 하는 일은 전부 터미널에서도 됩니다. 바이브 코딩은 터미널에서
일어나므로, `kigtit`은 앱보다 CLI가 먼저였습니다.

앱 사용법은 [README](../README.md)를 보세요.

## 설치

```sh
cargo install --path crates/cli
```

## 한눈에

```sh
kigtit                    # 지금 폴더의 타임라인
kigtit open               # 이 폴더를 앱 창으로 열기
kigtit watch              # 자동 저장 켜기
kigtit save "메모"         # 직접 담기
kigtit show <id> --code   # 무엇이 바뀌었는지
kigtit undo               # 마지막 세이브 포인트 이전으로
kigtit back <id>          # 특정 시점으로
kigtit health             # 앱이 켜지는지 돌려 보고 기록
kigtit mark ok|broken     # 판정을 직접 고쳐 쓰기
kigtit check --fix        # 비밀 키·대용량 파일 검사
kigtit backup             # GitHub에 백업 (기본: 비공개)
kigtit sync               # GitHub 쪽 변경 가져와 맞추기
kigtit summarize          # 요약이 빠진 것들 채우기
```

`-C <폴더>`를 붙이면 다른 폴더를 대상으로 합니다.

## 명령 하나하나

### `kigtit` — 타임라인

인수 없이 치면 지금 폴더의 타임라인이 나옵니다.

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

읽는 법:

| 표시 | 뜻 |
|---|---|
| `●` 초록 | 이 시점에서 앱이 잘 켜졌다 |
| `■` 빨강 | **여기서부터 앱이 안 켜졌다** — 아래 줄에 이유와 돌아갈 곳이 나온다 |
| `○` 회색 | 아직 확인하지 않았다 |
| `◆` 자주 | 아직 담기지 않은 변경 |

색만으로 말하지 않습니다. 도형이 같이 붙어서 색맹 사용자와 흑백 출력에서도
구분됩니다.

```sh
kigtit list --limit 30    # 더 많이 보기
```

### `kigtit watch` — 자동 저장

```sh
kigtit watch              # 3초 유휴 후 자동 저장
kigtit watch --idle 10    # 10초로 늘리기
```

끄려면 `Ctrl+C`. 강제로 꺼도 괜찮습니다 — 다시 켜면 놓친 요약을 이어서 채웁니다.

### `kigtit save` — 직접 담기

```sh
kigtit save                    # 제목을 AI가 붙여준다
kigtit save "메뉴 사진 추가"     # 제목을 직접 쓴다
kigtit save --no-summary       # AI 요약을 기다리지 않는다
```

```
$ kigtit save
  ●  저장했어요 2715681  자동 저장 · 파일 1개
     Claude Code로 요약하는 중…
     주문 버튼 디자인 변경
     주문하기 버튼이 초록색에서 파란색으로 바뀌었습니다. 버튼의 모서리가
     더 둥글어지고 크기가 커졌으며, 그림자 효과가 추가되었습니다.
```

### `kigtit show` — 무엇이 바뀌었는지

```sh
kigtit show                  # 가장 최근 것
kigtit show 801b8bf          # 특정 시점
kigtit show 801b8bf --code   # 코드까지 펼치기
```

사람 말 설명이 위에 오고, 코드는 `--code`를 줄 때만 나옵니다.

### `kigtit undo` / `kigtit back` — 되돌리기

```sh
kigtit undo               # 마지막 세이브 포인트 이전으로
kigtit back 2715681       # 특정 시점으로
```

![되돌리기](demo/undo.gif)

```
$ kigtit back 2715681
  ●  주문 버튼 디자인 변경 시점으로 되돌렸어요 2715681
     되돌리기 직전 상태는 c871216에 담아뒀어요.
     되돌린 것도 되돌릴 수 있어요 → kigtit undo
```

작업 중이던 미저장 변경이 있어도 괜찮습니다. 되돌리기 전에 먼저 담아 두므로
잃는 것이 없습니다.

### `kigtit health` — 앱이 켜지는지 확인

저장할 때 알아서 돕니다. 지금 당장 다시 보고 싶을 때만 씁니다.

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

판정이 틀렸으면 직접 고쳐 쓸 수 있습니다.

```sh
kigtit mark ok
kigtit mark broken
kigtit mark unknown 801b8bf
```

### `kigtit check` — 위험한 파일

```sh
kigtit check          # 검사만
kigtit check --fix    # 대용량 파일을 백업에서 바로 빼기
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

값 전체는 화면에 절대 나오지 않습니다.

### `kigtit backup` — GitHub에 백업

```sh
kigtit backup             # 비공개로 (기본)
kigtit backup --public    # 누구나 볼 수 있게
kigtit backup --status    # 올리지 않고 상태만
```

```
$ kigtit backup
  ●  yunhwane 계정으로 올릴 수 있어요
     아직 연결된 곳이 없어요. 새로 만들어 드릴게요.
     백업 안 된 세이브 포인트 2개

  나만 볼 수 있게 올립니다.
  올리는 중…
  ●  백업했어요 — 세이브 포인트 2개
     https://github.com/yunhwane/내-프로젝트.git
     저장소를 새로 만들었어요.
```

키가 하나라도 있으면 **올리지 않습니다.**

![키 유출 차단](demo/secret.gif)

### `kigtit sync` — GitHub와 맞추기

다른 컴퓨터에서 한 작업을 가져옵니다.

```sh
kigtit sync                 # 가져와서 맞추기
kigtit sync --keep mine     # 겹친 파일을 내 것으로
kigtit sync --keep theirs   # 겹친 파일을 GitHub 것으로
```

겹치면 멈추고 물어봅니다. **이때 작업 폴더는 건드리지 않습니다.**

![선택이 필요해요](demo/sync.gif)

### `kigtit open` — 앱 창으로 열기

```sh
kigtit open
```

터미널에서 작업하다가 눈으로 보고 싶을 때. 앱이 그 폴더로 바로 들어갑니다.

### `kigtit summarize` — 요약 채우기

```sh
kigtit summarize
kigtit summarize --limit 50
```

`--no-summary`로 담았거나 앱이 갑자기 꺼져서 빠진 요약을 채웁니다.

---

## 만들면서

```sh
cargo test -p kigtit-core        # 유닛 테스트
cargo clippy                     # 린트
cargo tauri dev                   # 앱을 개발 모드로
pnpm build                        # 프런트엔드만
```

`cargo run`으로 앱을 직접 띄우면 빈 창이 뜹니다 — debug 빌드의 Tauri는
`devUrl`(localhost:1420)을 보기 때문입니다. `cargo tauri dev`를 쓰거나 release로
구워야 임베드된 화면을 씁니다.

DMG 굽기는 Finder 권한이 필요해서 기본 대상에서 뺐습니다. 필요하면
`src-tauri/tauri.conf.json`의 `bundle.targets`에 `"dmg"`를 넣으세요.

### 화면과 GIF 다시 만들기

둘 다 손으로 찍지 않습니다. 대본이 저장소에 있어서 누구나 같은 결과를
다시 만들 수 있습니다.

```sh
brew install vhs
brew install --cask font-d2coding   # 한글 고정폭 폰트
cd demo && ./record.sh              # 터미널 GIF

pnpm build && node demo/shots.mjs   # 앱 화면 PNG
```

`demo/shots.mjs`가 찍는 것은 `src/`의 **실제 화면 코드**입니다. 목업이 아니고,
백엔드 자리에만 예시 데이터를 물려 줍니다.
