#!/usr/bin/env bash
# GIF를 찍기 전에 쓸 예시 프로젝트를 만든다.
# 요약은 미리 채워 둔다 — 하나에 8초씩 걸려서 GIF에 담으면 기다림만 보인다.
set -e
ROOT="${1:?대상 폴더를 넘겨 주세요}"
rm -rf "$ROOT"; mkdir -p "$ROOT"

cat > "$ROOT/menu.py" <<'EOF'
MENU = [
    {"name": "Americano", "price": 4},
    {"name": "Latte", "price": 4.5},
]
EOF
printf '[project]\nname = "cafe"\nversion = "0.1.0"\n' > "$ROOT/pyproject.toml"
kigtit -C "$ROOT" save "Create menu list" --no-summary >/dev/null 2>&1
kigtit -C "$ROOT" health >/dev/null 2>&1

cat > "$ROOT/order.py" <<'EOF'
from menu import MENU

def label(item):
    return f"{item['name']} ${item['price']}"
EOF
kigtit -C "$ROOT" save "Create order display" --no-summary >/dev/null 2>&1
kigtit -C "$ROOT" health >/dev/null 2>&1

# AI가 괄호를 안 닫은 상황
cat > "$ROOT/cart.py" <<'EOF'
from menu import MENU

def total(items):
    return sum(i["price"] for i in items

def first(items):
    return items[0]["name"]
EOF
kigtit -C "$ROOT" save "Add cart feature" --no-summary >/dev/null 2>&1
kigtit -C "$ROOT" health >/dev/null 2>&1
