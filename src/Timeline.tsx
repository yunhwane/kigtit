import {
  HEALTH_GLYPH,
  HEALTH_LABEL,
  type FileChange,
  type SavePoint,
  type View,
} from "./api";

interface Props {
  view: View;
  selected: string | null;
  onSelect: (id: string) => void;
  onRestore: (id: string) => void;
  projectName: string;
  agentLabel: string;
  activity: string | null;
}

export function Timeline({
  view,
  selected,
  onSelect,
  onRestore,
  projectName,
  agentLabel,
  activity,
}: Props) {
  const { points, pending } = view;

  return (
    <section className="stream">
      <div className="drag" />
      <div className="stream-head">
        <div>
          <h2>Work in {projectName}</h2>
          <div className="sub">
            {points.length} save points · Summaries by {agentLabel}
          </div>
        </div>
        <div className="autosave">
          <span className="pulse" />
          {activity ?? "Autosave on"}
        </div>
      </div>

      {pending.length > 0 && (
        <div className="ev" data-state="now">
          <div className="gutter">
            <span className="node" />
            <span className="stem" />
          </div>
          <div>
            <div className="card now">
              <div className="card-top">
                <span className="time">Now</span>
                <span className="chip accent">Saving</span>
              </div>
              <h3>{pending.length} unsaved changes</h3>
              <p>A save point will be created automatically in a few seconds.</p>
              <span className="meta">{names(pending)}</span>
            </div>
          </div>
        </div>
      )}

      {points.length === 0 && pending.length === 0 && (
        <p className="empty">
          No save points yet.
          <br />
          Change a file and it will be saved automatically.
        </p>
      )}

      {points.map((sp, i) => (
        <Event
          key={sp.full_id}
          sp={sp}
          selected={selected === sp.full_id}
          onSelect={() => onSelect(sp.full_id)}
          // 깨진 지점에서는 그 지점이 아니라 그보다 앞선, 마지막으로 잘
          // 켜졌던 시점을 가리켜야 쓸모가 있다.
          safe={sp.health === "broken" ? healthyBefore(points, i) : null}
          onRestore={onRestore}
        />
      ))}
    </section>
  );
}

function Event({
  sp,
  selected,
  onSelect,
  safe,
  onRestore,
}: {
  sp: SavePoint;
  selected: boolean;
  onSelect: () => void;
  safe: SavePoint | null;
  onRestore: (id: string) => void;
}) {
  return (
    <div className="ev" data-state={sp.health}>
      <div className="gutter">
        <span className="node" />
        <span className="stem" />
      </div>
      <div>
        <button className="card" aria-current={selected} onClick={onSelect}>
          <div className="card-top">
            <span className="time">{sp.at_label}</span>
            <span className={`chip ${sp.health}`} title={sp.checked_by ?? undefined}>
              <span className="glyph">{HEALTH_GLYPH[sp.health]}</span>
              {sp.health === "unknown" ? kindLabel(sp) : HEALTH_LABEL[sp.health]}
            </span>
          </div>
          <h3>{sp.title}</h3>
          {sp.summary ? (
            <p>{sp.summary}</p>
          ) : sp.pending_summary ? (
            <p className="waiting">Summarizing…</p>
          ) : null}
          <span className="meta">
            {sp.files.length} files · {kindLabel(sp)}
          </span>
        </button>

        {/* 왜 안 켜지는지 첫 줄만. 전체는 오른쪽 패널에서 본다. */}
        {sp.broke_because && (
          <p className="why">{sp.broke_because.split("\n")[0].trim()}</p>
        )}

        {safe && (
          <div className="row-actions">
            <button className="btn sm danger" onClick={() => onRestore(safe.full_id)}>
              Go back to {safe.title}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function healthyBefore(points: SavePoint[], i: number): SavePoint | null {
  return points.slice(i + 1).find((p) => p.health === "ok") ?? null;
}

function kindLabel(sp: SavePoint) {
  return { auto: "Autosave", manual: "Manual save", restore: "Restore", start: "Starting point" }[
    sp.kind
  ];
}

function names(files: FileChange[]) {
  const head = files.slice(0, 3).map((f) => f.path);
  const more = files.length - head.length;
  return more > 0 ? `${head.join(", ")} and ${more} more` : head.join(", ");
}
