/**
 * CHANGELOG.md 에서 해당 버전 절만 꺼내 릴리스 노트로 쓴다.
 *
 *   node scripts/release-notes.mjs 0.2.0
 *
 * 변경 기록을 두 곳에 따로 쓰면 반드시 어긋난다. CHANGELOG 하나만 관리한다.
 */
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const version = process.argv[2];
const body = await readFile(resolve(import.meta.dirname, "..", "CHANGELOG.md"), "utf8");

// `## [0.2.0]` 부터 다음 `## ` 직전까지.
const start = body.search(new RegExp(`^## \\[${version.replace(/\./g, "\\.")}\\]`, "m"));
if (start === -1) {
  console.log(`See CHANGELOG.md for what changed in ${version}.`);
  process.exit(0);
}
const rest = body.slice(start);
const next = rest.slice(1).search(/^## /m);
const section = (next === -1 ? rest : rest.slice(0, next + 1)).trim();

// 첫 줄(제목)은 GitHub 릴리스 제목과 겹치므로 뺀다.
console.log(section.split("\n").slice(1).join("\n").trim());
