import { useEffect, useState } from "react";
import { api, type Conflict, type Explanation, type Side } from "./api";

/**
 * "선택이 필요해요".
 *
 * 충돌은 비개발자가 포기하는 지점이다. 여기서 하는 일은 두 가지뿐이다.
 * 1. `<<<<<<< HEAD` 를 절대 보여주지 않는다. 파일마다 양쪽이 무엇을 하려
 *    했는지 사람 말로 설명하고, 어느 쪽을 남길지만 고르게 한다.
 * 2. 다 고를 때까지 작업 폴더를 건드리지 않는다. 그만둬도 잃는 게 없다.
 */
export function ConflictChoice({
  conflicts,
  onClose,
  onDone,
  say,
}: {
  conflicts: Conflict[];
  onClose: () => void;
  onDone: () => void;
  say: (message: string) => void;
}) {
  const [choices, setChoices] = useState<Record<string, Side>>({});
  const [why, setWhy] = useState<Record<string, Explanation | "loading">>({});
  const [busy, setBusy] = useState(false);

  // 설명이 곧 이 화면의 내용이다. 열자마자 받아 온다.
  useEffect(() => {
    for (const c of conflicts) {
      setWhy((prev) => ({ ...prev, [c.path]: "loading" }));
      api
        .syncExplain(c.path)
        .then((e) => setWhy((prev) => ({ ...prev, [c.path]: e })))
        .catch(() => setWhy((prev) => {
          const next = { ...prev };
          delete next[c.path];
          return next;
        }));
    }
  }, [conflicts]);

  const allChosen = conflicts.every((c) => choices[c.path]);

  async function apply() {
    setBusy(true);
    try {
      const list = conflicts.map((c) => [c.path, choices[c.path]] as [string, Side]);
      await api.syncResolve(list);
      say("선택한 대로 합쳤어요. 마음에 안 들면 되돌릴 수 있어요.");
      onDone();
      onClose();
    } catch (e) {
      say(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="scrim" onClick={busy ? undefined : onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <span className="chip warn">
          <span className="glyph">▲</span>선택이 필요해요
        </span>
        <h3>
          파일 {conflicts.length}개를 두 곳에서 같이 고쳤어요
        </h3>
        <p>
          어느 쪽을 남길지 골라 주세요. <b>다 고를 때까지 아무것도 바뀌지 않습니다.</b>
        </p>

        <div className="conflicts">
          {conflicts.map((c) => {
            const info = why[c.path];
            const picked = choices[c.path];
            return (
              <div className="conflict" key={c.path}>
                <code className="cpath">{c.path}</code>

                <div className="sides">
                  <button
                    className={picked === "mine" ? "side picked" : "side"}
                    onClick={() => setChoices((p) => ({ ...p, [c.path]: "mine" }))}
                    aria-pressed={picked === "mine"}
                  >
                    <span className="side-label">내 컴퓨터에서 한 것</span>
                    <span className="side-why">
                      {c.mine_deleted
                        ? "이 파일을 지웠어요."
                        : info === "loading"
                          ? "무엇을 했는지 읽는 중…"
                          : (info?.mine ?? "설명을 만들지 못했어요.")}
                    </span>
                  </button>

                  <button
                    className={picked === "theirs" ? "side picked" : "side"}
                    onClick={() => setChoices((p) => ({ ...p, [c.path]: "theirs" }))}
                    aria-pressed={picked === "theirs"}
                  >
                    <span className="side-label">GitHub에 있던 것</span>
                    <span className="side-why">
                      {c.theirs_deleted
                        ? "이 파일을 지웠어요."
                        : info === "loading"
                          ? "무엇을 했는지 읽는 중…"
                          : (info?.theirs ?? "설명을 만들지 못했어요.")}
                    </span>
                  </button>
                </div>
              </div>
            );
          })}
        </div>

        <div className="reassure">
          <span>🛟</span>
          <span>
            <b>고른 뒤에도 되돌릴 수 있어요.</b> 합치기도 하나의 세이브 포인트로
            남습니다.
          </span>
        </div>

        <div className="modal-actions">
          <button className="btn ghost" onClick={onClose} disabled={busy}>
            나중에 하기
          </button>
          <button className="btn primary" onClick={apply} disabled={busy || !allChosen}>
            {busy
              ? "합치는 중…"
              : allChosen
                ? "이대로 합치기"
                : `${conflicts.length - Object.keys(choices).length}개 더 골라 주세요`}
          </button>
        </div>
      </div>
    </div>
  );
}
