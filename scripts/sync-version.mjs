/**
 * release-it이 package.json 버전을 올린 뒤, 같은 버전을 나머지에도 적는다.
 *
 *   node scripts/sync-version.mjs 0.2.0
 *
 * 버전이 세 곳(package.json, Cargo 워크스페이스, tauri.conf.json)에 흩어져
 * 있어서 하나만 올리면 앱 정보창과 `kigtit --version`이 서로 다른 말을 한다.
 */
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(version ?? "")) {
  console.error("버전을 넘겨 주세요. 예: node scripts/sync-version.mjs 0.2.0");
  process.exit(1);
}

const ROOT = resolve(import.meta.dirname, "..");

async function edit(rel, fn) {
  const path = resolve(ROOT, rel);
  const before = await readFile(path, "utf8");
  const after = fn(before);
  if (before === after) {
    console.log(`  = ${rel} (이미 ${version})`);
    return;
  }
  await writeFile(path, after);
  console.log(`  ✓ ${rel} → ${version}`);
}

// [workspace.package] 아래 첫 version 만 바꾼다. 의존성 version은 건드리지 않는다.
await edit("Cargo.toml", (s) =>
  s.replace(
    /(\[workspace\.package\][^[]*?version\s*=\s*")[^"]+(")/,
    `$1${version}$2`,
  ),
);

await edit("src-tauri/tauri.conf.json", (s) =>
  s.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`),
);

// Cargo.lock 도 맞춰 둔다. 실패해도 릴리스를 막지 않는다.
const { spawnSync } = await import("node:child_process");
const lock = spawnSync("cargo", ["update", "--workspace", "--offline"], {
  cwd: ROOT,
  stdio: "ignore",
});
console.log(lock.status === 0 ? "  ✓ Cargo.lock" : "  ! Cargo.lock 은 직접 갱신해 주세요");
