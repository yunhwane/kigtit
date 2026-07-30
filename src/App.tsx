import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  api,
  onWatch,
  type Finding,
  type Health,
  type ProjectInfo,
  type Recent,
  type SavePoint,
  type View,
} from "./api";
import { Detail } from "./Detail";
import { Onboarding } from "./Onboarding";
import { Timeline } from "./Timeline";

export function App() {
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [view, setView] = useState<View | null>(null);
  const [recents, setRecents] = useState<Recent[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [revertTo, setRevertTo] = useState<SavePoint | null>(null);
  const [blocked, setBlocked] = useState<Finding[] | null>(null);
  const [summarizing, setSummarizing] = useState<string | null>(null);
  const [activity, setActivity] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const say = useCallback((message: string) => {
    setToast(message);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setToast(null), 3600);
  }, []);

  const refresh = useCallback(async () => {
    try {
      setView(await api.view());
    } catch {
      /* 프로젝트가 아직 열리지 않았을 뿐이다. */
    }
  }, []);

  const openProject = useCallback(
    async (path: string) => {
      try {
        const info = await api.openProject(path);
        setProject(info);
        setSelected(null);
        setBlocked(null);
        await refresh();
        api.recent().then(setRecents).catch(() => {});
      } catch (e) {
        say(String(e));
      }
    },
    [refresh, say],
  );

  // `kigtit`을 터미널에서 쳐서 띄웠다면 그 폴더로 바로 들어간다.
  useEffect(() => {
    api
      .launchFolder()
      .then((path) => {
        if (path) openProject(path);
      })
      .catch(() => {});
    api.recent().then(setRecents).catch(() => {});
  }, [openProject]);

  // 자동 저장이 알려오는 것들.
  useEffect(() => {
    return onWatch({
      changed: (files) => setActivity(`파일 ${files}개 바뀜 — 곧 담아요`),
      saved: (point) => {
        setActivity(null);
        say(`담았어요 · ${point.title}`);
        refresh();
      },
      blocked: (findings) => {
        setActivity(null);
        setBlocked(findings);
      },
      summarized: () => refresh(),
    });
  }, [refresh, say]);

  const point = view?.points.find((p) => p.full_id === selected) ?? view?.points[0] ?? null;

  async function confirmRestore() {
    if (!revertTo) return;
    const target = revertTo;
    setRevertTo(null);
    try {
      const done = await api.restoreTo(target.full_id);
      setSelected(done.created.full_id);
      await refresh();
      say(
        done.snapshot_id
          ? `되돌렸어요. 직전 상태는 ${done.snapshot_id}에 담아뒀어요.`
          : "되돌렸어요. 되돌린 것도 되돌릴 수 있어요.",
      );
    } catch (e) {
      say(String(e));
    }
  }

  async function mark(id: string, health: Health) {
    try {
      await api.mark(id, health);
      await refresh();
    } catch (e) {
      say(String(e));
    }
  }

  async function summarize(id: string) {
    setSummarizing(id);
    try {
      await api.summarize(id);
      await refresh();
    } catch (e) {
      say(String(e));
    } finally {
      setSummarizing(null);
    }
  }

  async function fixFinding(f: Finding) {
    try {
      await api.exclude(f.path);
      setBlocked((prev) => (prev ?? []).filter((x) => x.path !== f.path));
      say(`${f.path}를 백업에서 뺐어요.`);
    } catch (e) {
      say(String(e));
    }
  }

  if (!project || !view) {
    return (
      <>
        <div className="drag" style={{ position: "fixed", top: 0, left: 0, right: 0 }} />
        <Onboarding onOpen={openProject} />
        {toast && <div className="toast">{toast}</div>}
      </>
    );
  }

  return (
    <>
      <div className="app">
        <aside className="rail">
          <div className="drag" />
          <div className="rail-label">내 프로젝트</div>
          {recents.map((r) => (
            <button
              key={r.root}
              className={r.root === project.root ? "proj active" : "proj"}
              onClick={() => r.root !== project.root && openProject(r.root)}
              title={r.root}
            >
              <span className="swatch" />
              <span className="name">{r.name}</span>
            </button>
          ))}

          <div className="rail-foot">
            {project.agent === "rules" && (
              <p className="agent-note">
                Claude Code나 Codex를 설치하면 바뀐 내용을 사람 말로 설명해 드려요.
              </p>
            )}
            <button
              className="btn ghost sm wide"
              onClick={async () => {
                const path = await open({ directory: true, title: "폴더 고르기" });
                if (typeof path === "string") openProject(path);
              }}
            >
              ＋ 폴더 열기
            </button>
            <button
              className="btn ghost sm wide"
              onClick={async () => {
                const findings = await api.check();
                if (findings.length === 0) say("위험한 파일이 없어요.");
                else setBlocked(findings);
              }}
            >
              위험한 파일 검사
            </button>
          </div>
        </aside>

        <Timeline
          view={view}
          selected={point?.full_id ?? null}
          onSelect={setSelected}
          onRestore={(id) =>
            setRevertTo(view.points.find((p) => p.full_id === id) ?? null)
          }
          projectName={project.name}
          agentLabel={project.agent_label}
          activity={activity}
        />

        <Detail
          point={point}
          onRestore={(id) =>
            setRevertTo(view.points.find((p) => p.full_id === id) ?? null)
          }
          onMark={mark}
          onSummarize={summarize}
          summarizing={summarizing === point?.full_id}
        />
      </div>

      {revertTo && (
        <div className="scrim" onClick={() => setRevertTo(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <span className="chip accent">되돌리기</span>
            <h3>
              {revertTo.at_label} “{revertTo.title}” 시점으로 되돌릴까요?
            </h3>
            <p>이 시점 이후에 바뀐 내용은 화면에서 사라집니다.</p>
            {/* 이 한 줄이 비개발자가 버튼을 누르게 만든다. */}
            <div className="reassure">
              <span>🛟</span>
              <span>
                <b>지금 상태는 자동으로 저장됩니다.</b> 되돌린 것도 되돌릴 수 있어요. 무엇도
                영구히 사라지지 않습니다.
              </span>
            </div>
            <div className="modal-actions">
              <button className="btn ghost" onClick={() => setRevertTo(null)}>
                그만두기
              </button>
              <button className="btn primary" onClick={confirmRestore}>
                되돌리기
              </button>
            </div>
          </div>
        </div>
      )}

      {blocked && blocked.length > 0 && (
        <div className="scrim">
          <div className="modal">
            <span className="chip warn">
              <span className="glyph">▲</span>
              {blocked.some((f) => f.risk === "secret") ? "저장을 멈췄어요" : "확인이 필요해요"}
            </span>
            <h3>
              {blocked.some((f) => f.risk === "secret")
                ? "비밀 키처럼 보이는 게 파일에 들어 있어요"
                : "백업에 넣으면 곤란한 파일이 있어요"}
            </h3>
            <div className="findings">
              {blocked.map((f) => (
                <div className="finding" key={f.path + f.line}>
                  <span>{f.message}</span>
                  {f.masked && <code>{f.masked}</code>}
                  <div className="row-actions" style={{ margin: 0 }}>
                    <button className="btn sm" onClick={() => fixFinding(f)}>
                      백업에서 빼두기
                    </button>
                  </div>
                </div>
              ))}
            </div>
            <div className="reassure warn">
              <span>🔐</span>
              <span>
                키는 <b>.env 파일</b>로 옮기고, 코드에서는 그 파일을 불러오도록 바꿔 주세요.
                .env는 이미 백업에서 빠져 있습니다.
              </span>
            </div>
            <div className="modal-actions">
              <button
                className="btn primary"
                onClick={() => {
                  setBlocked(null);
                  refresh();
                }}
              >
                알겠어요
              </button>
            </div>
          </div>
        </div>
      )}

      {toast && <div className="toast">{toast}</div>}
    </>
  );
}
