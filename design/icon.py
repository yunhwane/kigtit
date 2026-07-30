"""Kigtit 앱 아이콘. 표준 라이브러리만으로 PNG를 쓴다.

그림: 자주색 바탕에 세로 레일과 노드 세 개. 타임라인이 이 앱의 도구다.
가운데 노드만 채워서 "지금 여기" 를 나타낸다. 4배 확대해 그린 뒤 평균을
내는 방식으로 계단을 없앤다.
"""

import struct
import zlib

S = 1024          # 최종 크기
SS = 4            # 슈퍼샘플링 배수
W = S * SS

PLUM = (0x6B, 0x2D, 0x5C)
PAPER = (0xFA, 0xF7, 0xF8)
SAGE = (0x7D, 0xC5, 0xA0)


def rounded_rect(x, y, w, h, r, px, py):
    """둥근 사각형 안에 있는지. 가장 가까운 코너 중심까지의 거리로 판정한다."""
    dx = max(x + r - px, 0.0, px - (x + w - r))
    dy = max(y + r - py, 0.0, py - (y + h - r))
    return dx * dx + dy * dy <= r * r


def ring(cx, cy, outer, inner, px, py):
    d2 = (px - cx) ** 2 + (py - cy) ** 2
    return inner * inner <= d2 <= outer * outer


def disc(cx, cy, r, px, py):
    return (px - cx) ** 2 + (py - cy) ** 2 <= r * r


# ── 도형 배치 (최종 좌표 기준, 1024 캔버스) ──────────────
PAD = 84
RAIL_X = S * 0.5
RAIL_W = 36
NODE_R = 90
NODE_T = 36                     # 테두리 두께
NODES = [S * 0.24, S * 0.5, S * 0.76]
SAGE_I = 2                      # 아래쪽 = 마지막으로 잘 켜졌던 시점


def sample(px, py):
    """한 점의 색. 아이콘 바깥은 None."""
    if not rounded_rect(PAD, PAD, S - 2 * PAD, S - 2 * PAD, 200, px, py):
        return None

    # 노드가 레일을 덮는다. 순서가 뒤집히면 레일이 노드를 가른다.
    for i, cy in enumerate(NODES):
        if i == SAGE_I:
            if disc(RAIL_X, cy, NODE_R, px, py):
                return SAGE
        else:
            if ring(RAIL_X, cy, NODE_R, NODE_R - NODE_T, px, py):
                return PAPER
            if disc(RAIL_X, cy, NODE_R - NODE_T, px, py):
                return PLUM

    # 레일: 첫 노드와 끝 노드 사이만.
    if abs(px - RAIL_X) <= RAIL_W / 2 and NODES[0] <= py <= NODES[-1]:
        return PAPER

    return PLUM


rows = []
for y in range(S):
    row = bytearray()
    for x in range(S):
        rs = gs = bs = as_ = 0
        for sy in range(SS):
            for sx in range(SS):
                px = x + (sx + 0.5) / SS
                py = y + (sy + 0.5) / SS
                c = sample(px, py)
                if c is None:
                    continue
                rs += c[0]
                gs += c[1]
                bs += c[2]
                as_ += 255
        n = SS * SS
        if as_ == 0:
            row += bytes((0, 0, 0, 0))
        else:
            # 색은 덮인 부분만 평균, 알파는 전체 대비 비율.
            covered = as_ // 255
            row += bytes((rs // covered, gs // covered, bs // covered, as_ // n))
    rows.append(bytes(row))

raw = b"".join(b"\x00" + r for r in rows)


def chunk(tag, data):
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))


png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", S, S, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw, 9))
       + chunk(b"IEND", b""))

import sys
open(sys.argv[1], "wb").write(png)
print(f"{sys.argv[1]}  {S}x{S}  {len(png) // 1024}KB")
