/**
 * 앱 화면 스크린샷을 만든다.
 *
 *   pnpm build && node demo/shots.mjs
 *
 * 찍는 대상은 **src/ 의 실제 React 화면 코드**다. 목업이 아니다. 다만 Tauri
 * 창이 아니라 헤드리스 브라우저에 렌더하고, 백엔드 대신 아래 SAMPLE 데이터를
 * 물려 준다. 화면 코드·CSS·문구는 앱에서 돌아가는 것과 같은 것이고, 데이터만
 * 예시다. 실제 창을 그대로 찍으려면 화면 기록 권한이 필요하다.
 *
 * SAMPLE은 백엔드가 실제로 돌려주는 모양(src/api.ts)을 그대로 따른다.
 */

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";
import { chromium } from "playwright";

const ROOT = resolve(import.meta.dirname, "..");
/** "최근에 열었던 폴더"가 에포크(1970년)로 보이지 않게 지금 기준으로 잡는다. */
const NOW = Math.floor(Date.now() / 1000);
const DIST = join(ROOT, "dist");
const OUT = join(ROOT, "demo");

// ── 예시 데이터 ────────────────────────────────────────────

const sp = (o) => ({
  summary: null,
  checked_by: null,
  broke_because: null,
  files: [],
  pending_summary: false,
  kind: "auto",
  health: "unknown",
  ...o,
});

const POINTS = [
  sp({
    id: "801b8bf",
    full_id: "801b8bf0000000000000000000000000000000aa",
    at: 0,
    at_label: "오후 3:12",
    title: "장바구니 계산 추가",
    summary:
      "장바구니에 담긴 상품들의 총 가격을 계산하고 첫 번째 상품의 이름을 보여주는 기능이 추가되었습니다.",
    health: "broken",
    checked_by: "앱 빌드",
    broke_because:
      "*** Error compiling './cart.py'...\n  File \"./cart.py\", line 4\n    return sum(i[\"price\"] for i in items\n              ^\nSyntaxError: '(' was never closed",
    files: [
      { path: "components/Cart.tsx", kind: "새 파일", added: 14, removed: 0 },
      { path: "lib/cart.ts", kind: "새 파일", added: 22, removed: 0 },
      { path: "app/page.tsx", kind: "수정", added: 3, removed: 1 },
    ],
  }),
  sp({
    id: "2715681",
    full_id: "27156810000000000000000000000000000000bb",
    at: 0,
    at_label: "오후 2:47",
    title: "메뉴 카드에 사진 넣기",
    summary:
      "메뉴 카드 위쪽에 사진 영역을 새로 만들었어요. 사진이 아직 없는 메뉴에는 회색 기본 이미지가 대신 보입니다.",
    health: "ok",
    checked_by: "앱 빌드",
    files: [
      { path: "components/MenuCard.tsx", kind: "수정", added: 22, removed: 3 },
      { path: "lib/menu.ts", kind: "수정", added: 8, removed: 0 },
      { path: "public/menu-placeholder.png", kind: "새 파일", added: 0, removed: 0 },
    ],
  }),
  sp({
    id: "9f3c204",
    full_id: "9f3c2040000000000000000000000000000000cc",
    at: 0,
    at_label: "오후 2:31",
    title: "주문 버튼 색 바꾸기",
    summary:
      "주문하기 버튼이 초록색에서 파란색으로 바뀌었습니다. 모서리가 더 둥글어지고 그림자 효과가 추가되었습니다.",
    health: "ok",
    checked_by: "앱 빌드",
    files: [{ path: "components/OrderButton.tsx", kind: "수정", added: 4, removed: 4 }],
  }),
  sp({
    id: "da8d2b9",
    full_id: "da8d2b90000000000000000000000000000000dd",
    at: 0,
    at_label: "오후 1:20",
    title: "프로젝트 시작",
    kind: "start",
    files: [{ path: "app/page.tsx", kind: "새 파일", added: 24, removed: 0 }],
  }),
];

const SAMPLE = {
  launch_folder: "/Users/yunhwane/Documents/카페-주문앱",
  open_project: {
    root: "/Users/yunhwane/Documents/카페-주문앱",
    name: "카페 주문앱",
    agent: "claude",
    agent_label: "Claude Code",
    has_history: true,
  },
  recent: [
    { root: "/Users/yunhwane/Documents/카페-주문앱", name: "카페 주문앱", at: NOW - 600 },
    { root: "/Users/yunhwane/Documents/포트폴리오", name: "포트폴리오 사이트", at: NOW - 86400 },
    { root: "/Users/yunhwane/Documents/독서기록", name: "독서 기록장", at: NOW - 3 * 86400 },
  ],
  view: {
    points: POINTS,
    pending: [
      { path: "app/page.tsx", kind: "수정", added: 6, removed: 2 },
      { path: "lib/menu.ts", kind: "수정", added: 2, removed: 0 },
    ],
    last_healthy: "2715681",
  },
  health_probe: "앱 빌드",
  backup_status: {
    readiness: { state: "ready", account: "yunhwane" },
    remote: null,
    unbacked: 12,
    branch: "main",
  },
  backup_guard: [],
  sync_now: {
    kind: "needsChoice",
    conflicts: [
      { path: "config.js", mine_deleted: false, theirs_deleted: false },
      { path: "lib/menu.ts", mine_deleted: false, theirs_deleted: false },
    ],
  },
  sync_explain: {
    "config.js": {
      path: "config.js",
      mine: "카페의 영업시간이 밤 10시까지에서 오후 6시까지로 변경되어 더 일찍 닫게 됩니다. 일요일에 문을 닫는다는 정보가 새로 추가되었습니다.",
      theirs:
        "카페 이름이 '내 카페 ☕'로 바뀌고, 영업시간이 새벽 7시부터 밤 11시까지로 늘어났습니다. 새벽 배송이 시작되었다는 공지가 추가되었습니다.",
    },
    "lib/menu.ts": {
      path: "lib/menu.ts",
      mine: "메뉴에서 콜드브루를 빼고 대신 디카페인 라떼를 넣었습니다. 가격은 5000원입니다.",
      theirs: "메뉴에 콜드브루와 티 두 가지가 더해져 모두 네 가지가 되었습니다.",
    },
  },
};

/** 백엔드 대신 응답한다. `src/api.ts`의 명령 이름을 그대로 받는다. */
function handler(sample) {
  return (cmd, args) => {
    // 이벤트 구독은 등록만 받고 아무것도 보내지 않는다.
    if (cmd.startsWith("plugin:event|")) return 1;
    if (cmd.startsWith("plugin:")) return null;
    if (cmd === "sync_explain") return sample.sync_explain[args.path] ?? null;
    return cmd in sample ? sample[cmd] : null;
  };
}

// ── 정적 서버 ──────────────────────────────────────────────

const MIME = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

function serve(dir) {
  const server = createServer(async (req, res) => {
    const path = req.url.split("?")[0];
    const file = join(dir, path === "/" ? "index.html" : path);
    try {
      const body = await readFile(file);
      res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" });
      res.end(body);
    } catch {
      res.writeHead(404).end("not found");
    }
  });
  return new Promise((ok) => server.listen(0, () => ok(server)));
}

// ── 찍기 ──────────────────────────────────────────────────

const SHOTS = [
  {
    name: "app-timeline",
    what: "메인 화면 — 노드 하나가 내가 시킨 일 한 번",
    async act() {},
  },
  {
    name: "app-broken",
    what: "앱이 안 켜지는 시점을 고르면 이유가 그대로 나온다",
    async act(page) {
      await page.getByRole("button", { name: /장바구니 계산 추가/ }).click();
      await page.waitForTimeout(200);
    },
  },
  {
    name: "app-revert",
    what: "되돌리기 확인창 — 안심시키는 한 줄이 핵심",
    async act(page) {
      await page.getByRole("button", { name: /장바구니 계산 추가/ }).click();
      await page.getByRole("button", { name: "이 시점으로 되돌리기" }).click();
      await page.waitForTimeout(200);
    },
  },
  {
    name: "app-backup",
    what: "GitHub 백업 — 토큰을 묻지 않고 기본은 비공개",
    async act(page) {
      await page.getByRole("button", { name: "GitHub에 백업" }).click();
      await page.waitForTimeout(400);
    },
  },
  {
    name: "app-conflict",
    what: "선택이 필요해요 — 양쪽이 뭘 하려 했는지 읽고 고른다",
    async act(page) {
      await page.getByRole("button", { name: "GitHub와 맞추기" }).click();
      await page.waitForTimeout(600);
      await page.getByRole("button", { name: /영업시간이 밤 10시까지에서/ }).click();
      await page.waitForTimeout(200);
    },
  },
  {
    name: "app-start",
    what: "첫 실행 — 폴더 하나. 계정도 설정도 없다",
    sample: { ...SAMPLE, launch_folder: null },
    async act() {},
  },
];

const server = await serve(DIST);
const base = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch();

for (const shot of SHOTS) {
  const page = await browser.newPage({
    viewport: { width: 1120, height: 720 },
    deviceScaleFactor: 2,
    colorScheme: "dark",
  });

  // 화면이 뜨기 전에 백엔드 자리를 채운다.
  await page.addInitScript(
    ([sample, src]) => {
      const handle = new Function("return " + src)()(sample);
      window.__TAURI_INTERNALS__ = {
        invoke: (cmd, args) => Promise.resolve(handle(cmd, args ?? {})),
        transformCallback: (cb) => {
          const id = Math.floor(Math.random() * 1e9);
          window[`_cb_${id}`] = cb;
          return id;
        },
      };
    },
    [shot.sample ?? SAMPLE, handler.toString()],
  );

  await page.goto(base, { waitUntil: "networkidle" });
  await page.waitForTimeout(500);
  await shot.act(page);

  await page.screenshot({ path: join(OUT, `${shot.name}.png`) });
  console.log(`  ${shot.name}.png  ${shot.what}`);
  await page.close();
}

await browser.close();
server.close();
