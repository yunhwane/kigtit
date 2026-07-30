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
          <h2>{projectName}에서 한 일</h2>
          <div className="sub">
            세이브 포인트 {points.length}개 · 요약 {agentLabel}
          </div>
        </div>
        <div className="autosave">
          <span className="pulse" />
          {activity ?? "자동 저장 켜짐"}
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
                <span className="time">지금</span>
                <span className="chip accent">저장 중</span>
              </div>
              <h3>아직 저장되지 않은 변경 {pending.length}개</h3>
              <p>몇 초 뒤에 자동으로 세이브 포인트가 만들어져요.</p>
              <span className="meta">{names(pending)}</span>
            </div>
          </div>
        </div>
      )}

      {points.length === 0 && pending.length === 0 && (
        <p className="empty">
          아직 세이브 포인트가 없어요.
          <br />
          파일을 바꾸면 알아서 담깁니다.
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
            <span className={`chip ${sp.health}`}>
              <span className="glyph">{HEALTH_GLYPH[sp.health]}</span>
              {sp.health === "unknown" ? kindLabel(sp) : HEALTH_LABEL[sp.health]}
            </span>
          </div>
          <h3>{sp.title}</h3>
          {sp.summary ? (
            <p>{sp.summary}</p>
          ) : sp.pending_summary ? (
            <p className="waiting">요약 중…</p>
          ) : null}
          <span className="meta">
            파일 {sp.files.length}개 · {kindLabel(sp)}
          </span>
        </button>

        {safe && (
          <div className="row-actions">
            <button className="btn sm danger" onClick={() => onRestore(safe.full_id)}>
              {safe.title} 시점으로 되돌리기
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
  return { auto: "자동 저장", manual: "직접 저장", restore: "되돌림", start: "시작점" }[
    sp.kind
  ];
}

function names(files: FileChange[]) {
  const head = files.slice(0, 3).map((f) => f.path);
  const more = files.length - head.length;
  return more > 0 ? `${head.join(", ")} 외 ${more}개` : head.join(", ");
}
