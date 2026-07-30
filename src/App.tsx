import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  api,
  onWatch,
  type Conflict,
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
import { Backup } from "./Backup";
import { ConflictChoice } from "./Conflict";

export function App() {
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [view, setView] = useState<View | null>(null);
  const [recents, setRecents] = useState<Recent[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [revertTo, setRevertTo] = useState<SavePoint | null>(null);
  const [blocked, setBlocked] = useState<Finding[] | null>(null);
  const [summarizing, setSummarizing] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [probe, setProbe] = useState<string | null>(null);
  const [backupOpen, setBackupOpen] = useState(false);
  const [conflicts, setConflicts] = useState<Conflict[] | null>(null);
  const [syncing, setSyncing] = useState(false);
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
        api.healthProbe().then(setProbe).catch(() => setProbe(null));
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
      changed: (files) => setActivity(`${files} files changed — saving soon`),
      saved: (point) => {
        setActivity("Checking whether the app starts…");
        say(`Saved · ${point.title}`);
        refresh();
      },
      checked: (_id, outcome) => {
        setActivity(null);
        if (outcome.health === "broken") say(`The app won't start — ${outcome.how}`);
        refresh();
      },
      blocked: (findings) => {
        setActivity(null);
        setBlocked(findings);
      },
      summarized: () => refresh(),
      resuming: (count) => say(`Finishing ${count} summaries missed while Kigtit was closed.`),
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
          ? `Restored. The previous state was saved as ${done.snapshot_id}.`
          : "Restored. You can undo this restore too.",
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

  async function runCheck() {
    setChecking(true);
    try {
      const outcome = await api.checkHealth();
      await refresh();
      say(
        outcome.health === "ok"
          ? `The app starts fine · ${outcome.how}`
          : outcome.health === "broken"
            ? `The app won't start · ${outcome.how}`
            : outcome.how,
      );
    } catch (e) {
      say(String(e));
    } finally {
      setChecking(false);
    }
  }

  async function runSync() {
    setSyncing(true);
    try {
      const out = await api.sync();
      switch (out.kind) {
        case "noRemote":
          say("No GitHub repository is connected yet. Back up first.");
          break;
        case "upToDate":
          say("Already up to date.");
          break;
        case "pulled":
          say(`Pulled ${out.count} save points from GitHub.`);
          break;
        case "merged":
          say(`Merged automatically with no overlapping files. Applied ${out.count}.`);
          break;
        case "needsChoice":
          setConflicts(out.conflicts);
          break;
      }
      await refresh();
    } catch (e) {
      say(String(e));
    } finally {
      setSyncing(false);
    }
  }

  async function fixFinding(f: Finding) {
    try {
      await api.exclude(f.path);
      setBlocked((prev) => (prev ?? []).filter((x) => x.path !== f.path));
      say(`Excluded ${f.path} from backups.`);
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
          <div className="rail-label">My projects</div>
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
                Install Claude Code or Codex to get plain-language explanations of changes.
              </p>
            )}
            <button
              className="btn ghost sm wide"
              onClick={async () => {
                const path = await open({ directory: true, title: "Choose a folder" });
                if (typeof path === "string") openProject(path);
              }}
            >
              ＋ Open folder
            </button>
            <button className="btn ghost sm wide" onClick={() => setBackupOpen(true)}>
              Back up to GitHub
            </button>
            <button className="btn ghost sm wide" onClick={runSync} disabled={syncing}>
              {syncing ? "Syncing…" : "Sync with GitHub"}
            </button>
            <button
              className="btn ghost sm wide"
              onClick={async () => {
                const findings = await api.check();
                if (findings.length === 0) say("No risky files found.");
                else setBlocked(findings);
              }}
            >
              Scan for risky files
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
          onCheck={runCheck}
          checking={checking}
          probe={probe}
        />
      </div>

      {backupOpen && (
        <Backup onClose={() => setBackupOpen(false)} onDone={say} />
      )}

      {conflicts && conflicts.length > 0 && (
        <ConflictChoice
          conflicts={conflicts}
          onClose={() => setConflicts(null)}
          onDone={refresh}
          say={say}
        />
      )}

      {revertTo && (
        <div className="scrim" onClick={() => setRevertTo(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <span className="chip accent">Restore</span>
            <h3>
              Go back to “{revertTo.title}” at {revertTo.at_label}?
            </h3>
            <p>Changes made after this point will disappear from the current view.</p>
            {/* 이 한 줄이 비개발자가 버튼을 누르게 만든다. */}
            <div className="reassure">
              <span>🛟</span>
              <span>
                <b>Your current state will be saved automatically.</b> You can undo the restore.
                Nothing is permanently lost.
              </span>
            </div>
            <div className="modal-actions">
              <button className="btn ghost" onClick={() => setRevertTo(null)}>
                Cancel
              </button>
              <button className="btn primary" onClick={confirmRestore}>
                Restore
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
              {blocked.some((f) => f.risk === "secret") ? "Saving paused" : "Review needed"}
            </span>
            <h3>
              {blocked.some((f) => f.risk === "secret")
                ? "A file appears to contain a secret key"
                : "Some files should not be included in backups"}
            </h3>
            <div className="findings">
              {blocked.map((f) => (
                <div className="finding" key={f.path + f.line}>
                  <span>{f.message}</span>
                  {f.masked && <code>{f.masked}</code>}
                  <div className="row-actions" style={{ margin: 0 }}>
                    <button className="btn sm" onClick={() => fixFinding(f)}>
                      Exclude from backups
                    </button>
                  </div>
                </div>
              ))}
            </div>
            <div className="reassure warn">
              <span>🔐</span>
              <span>
                Move the key to an <b>.env file</b> and load it from your code.
                .env files are already excluded from backups.
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
                Got it
              </button>
            </div>
          </div>
        </div>
      )}

      {toast && <div className="toast">{toast}</div>}
    </>
  );
}
