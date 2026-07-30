#!/usr/bin/env bash
# README에 쓰는 GIF를 전부 다시 찍는다.
#
#   brew install vhs
#   brew install --cask font-d2coding    # 한글 고정폭 폰트
#   cargo install --path ../crates/cli
#   cd demo && ./record.sh
#
# 대본 일부는 되돌리기·합치기를 **실제로 실행**하므로 예시 프로젝트 상태를
# 바꾼다. 그래서 매 GIF 앞에서 씨를 새로 심는다. 안 그러면 다음 GIF에
# 이전 녹화의 흔적이 찍힌다.
set -euo pipefail
cd "$(dirname "$0")"

command -v kigtit >/dev/null || { echo "kigtit이 없습니다. cargo install --path ../crates/cli"; exit 1; }
command -v vhs    >/dev/null || { echo "vhs가 없습니다. brew install vhs"; exit 1; }

BASE=/tmp/kigtit-demo

# ── 씨 심기 ───────────────────────────────────────────────

seed_cafe() {
  # 요약을 미리 채워 둔다 — 하나에 8초씩 걸려서 GIF에 담으면 기다림만 보인다.
  ./_setup.sh "$BASE/cafe"
}

seed_leaky() {
  rm -rf "$BASE/leaky"; mkdir -p "$BASE/leaky"
  echo 'console.log("주문 앱")' > "$BASE/leaky/app.js"
  kigtit -C "$BASE/leaky" save "첫 화면" --no-summary >/dev/null
  # 이미 담긴 뒤에 키가 들어간 상황 = check는 놓치고 backup은 잡는다
  cat > "$BASE/leaky/config.js" <<'EOF'
export const KEY = "sk-proj-9aB3xQ7zLmN4pR8sT2vW1yU6iO0kJ5hG3fD2sA1qZ8xC7vB4nM"
EOF
  git -C "$BASE/leaky" add -A >/dev/null
  git -C "$BASE/leaky" -c user.email=demo@example.com -c user.name=demo \
      commit -qm "설정 추가" >/dev/null
}

seed_live() {
  rm -rf "$BASE/live"; mkdir -p "$BASE/live"
  printf '[project]\nname = "live"\nversion = "0.1.0"\n' > "$BASE/live/pyproject.toml"
  echo 'PRICE = 4000' > "$BASE/live/price.py"
  kigtit -C "$BASE/live" save "가격 정하기" --no-summary >/dev/null
}

seed_conflict() {
  rm -rf "$BASE/hub.git" "$BASE/laptop" "$BASE/desktop"
  git init -q --bare "$BASE/hub.git"
  mkdir -p "$BASE/laptop"
  printf 'export const TITLE = "내 카페"\nexport const HOURS = "10-22"\n' > "$BASE/laptop/config.js"
  kigtit -C "$BASE/laptop" save "첫 설정" --no-summary >/dev/null
  git -C "$BASE/laptop" remote add origin "$BASE/hub.git"
  git -C "$BASE/laptop" push -q --set-upstream origin main

  git clone -q "$BASE/hub.git" "$BASE/desktop"

  printf 'export const TITLE = "내 카페 ☕"\nexport const HOURS = "07-23"\nexport const NOTICE = "새벽 배송 시작"\n' \
    > "$BASE/laptop/config.js"
  kigtit -C "$BASE/laptop" save "노트북: 영업시간 확대, 공지 추가" --no-summary >/dev/null
  git -C "$BASE/laptop" push -q

  printf 'export const TITLE = "우리 동네 카페"\nexport const HOURS = "10-18"\nexport const CLOSED = "일요일 휴무"\n' \
    > "$BASE/desktop/config.js"
  kigtit -C "$BASE/desktop" save "데스크톱: 영업시간 줄이고 휴무 안내" --no-summary >/dev/null
}

# ── 녹화 ──────────────────────────────────────────────────

mkdir -p "$BASE"

echo "▸ timeline.gif  (읽기만 한다)"
seed_cafe
vhs timeline.tape

echo "▸ undo.gif  (되돌리기를 실제로 실행하므로 씨를 다시 심는다)"
seed_cafe
vhs undo.tape

echo "▸ secret.gif"
seed_leaky
vhs secret.tape

echo "▸ watch.gif"
seed_live
vhs watch.tape

echo "▸ sync.gif  (합치기를 실제로 실행한다)"
seed_conflict
vhs sync.tape

echo "▸ 완료"
ls -lh ./*.gif
