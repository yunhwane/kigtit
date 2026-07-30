import { useEffect, useState } from "react";
import { HEALTH_GLYPH, HEALTH_LABEL, api, type Health, type SavePoint } from "./api";

interface Props {
  point: SavePoint | null;
  onRestore: (id: string) => void;
  onMark: (id: string, health: Health) => void;
  onSummarize: (id: string) => void;
  summarizing: boolean;
  /** 앱이 켜지는지 지금 확인한다. 빌드를 돌리므로 오래 걸릴 수 있다. */
  onCheck: () => void;
  checking: boolean;
  /** 이 프로젝트를 무엇으로 확인하는지, 또는 왜 확인할 수 없는지. */
  probe: string | null;
}

/** 오른쪽 패널. 사람 말이 위, 코드는 접혀 있고 원할 때만 펼친다. */
export function Detail({
  point,
  onRestore,
  onMark,
  onSummarize,
  summarizing,
  onCheck,
  checking,
  probe,
}: Props) {
  const [patch, setPatch] = useState<string | null>(null);

  useEffect(() => {
    setPatch(null);
  }, [point?.full_id]);

  if (!point) {
    return (
      <aside className="detail">
        <div className="drag" />
        <p className="empty">Choose a save point on the left.</p>
      </aside>
    );
  }

  return (
    <aside className="detail">
      <div className="drag" />
      <div className="detail-head">
        <span className={`chip ${point.health}`}>
          <span className="glyph">{HEALTH_GLYPH[point.health]}</span>
          {HEALTH_LABEL[point.health]}
        </span>
        <h3>{point.title}</h3>
        <span className="time">
          {point.at_label} · {point.files.length} files · {point.id}
          {point.checked_by && ` · ${point.checked_by}`}
        </span>
      </div>

      {/* 안 켜지는 이유. 사용자가 실제로 알아야 하는 유일한 기술적 정보다. */}
      {point.broke_because && (
        <div className="broke">
          <span className="tag">Why it won't start</span>
          <pre>{point.broke_because}</pre>
        </div>
      )}

      <div className="summary">
        <span className="tag">What changed</span>
        {point.summary ? (
          <p>{point.summary}</p>
        ) : (
          <>
            <p style={{ color: "var(--ink-3)" }}>
              {summarizing ? "Writing a plain-language summary… This takes about 8 seconds." : "No summary yet."}
            </p>
            {!summarizing && (
              <button className="btn sm" onClick={() => onSummarize(point.full_id)}>
                Create summary
              </button>
            )}
          </>
        )}
      </div>

      {point.files.length > 0 && (
        <div className="files">
          {point.files.map((f) => (
            <div className="file" key={f.path}>
              <code>{f.path}</code>
              <span className="k">{f.kind}</span>
              <span className="plusminus">
                <span className="p">+{f.added}</span> <span className="m">−{f.removed}</span>
              </span>
            </div>
          ))}
        </div>
      )}

      {patch === null ? (
        <button
          className="btn wide"
          onClick={() => api.patch(point.full_id).then(setPatch).catch(() => setPatch(""))}
        >
          Show code
        </button>
      ) : (
        <Patch text={patch} />
      )}

      {/* 판정은 자동으로 돈다. 이 버튼은 지금 당장 다시 보고 싶을 때만 쓴다. */}
      <button className="btn wide" onClick={onCheck} disabled={checking}>
        {checking ? "Checking…" : "Check whether the app starts now"}
      </button>
      {probe && <p className="probe">{probe}</p>}

      <div className="row-actions">
        <button
          className="btn sm"
          onClick={() => onMark(point.full_id, point.health === "ok" ? "unknown" : "ok")}
        >
          {point.health === "ok" ? "Clear starts-fine mark" : "The app starts here"}
        </button>
        <button
          className="btn sm"
          onClick={() => onMark(point.full_id, point.health === "broken" ? "unknown" : "broken")}
        >
          {point.health === "broken" ? "Clear won't-start mark" : "The app won't start here"}
        </button>
      </div>

      <button className="btn wide danger" onClick={() => onRestore(point.full_id)}>
        Go back to this point
      </button>
    </aside>
  );
}

function Patch({ text }: { text: string }) {
  if (!text.trim()) return <p className="empty">No code to show.</p>;
  return (
    <pre className="patch">
      {text.split("\n").map((line, i) => (
        <span key={i} className={lineClass(line)}>
          {line || " "}
        </span>
      ))}
    </pre>
  );
}

function lineClass(line: string) {
  if (line.startsWith("+++") || line.startsWith("---")) return "hunk";
  if (line.startsWith("+")) return "add";
  if (line.startsWith("-")) return "del";
  if (line.startsWith("@@") || line.startsWith("diff ") || line.startsWith("index "))
    return "hunk";
  return "";
}
