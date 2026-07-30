import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Recent } from "./api";

/** 첫 화면. 폴더 하나. 계정도, 설정도, 깃 설치 확인도 없다. */
export function Onboarding({ onOpen }: { onOpen: (path: string) => void }) {
  const [recents, setRecents] = useState<Recent[]>([]);
  const [over, setOver] = useState(false);

  useEffect(() => {
    api.recent().then(setRecents).catch(() => {});
  }, []);

  async function pick() {
    const path = await open({ directory: true, title: "작업하던 폴더를 고르세요" });
    if (typeof path === "string") onOpen(path);
  }

  return (
    <div
      className="onboard"
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={() => setOver(false)}
    >
      <div className={over ? "dropzone over" : "dropzone"}>
        <span className="glyph">🕰️</span>
        <h2>작업하던 폴더를 끌어다 놓으세요</h2>
        <p>나머지는 알아서 준비합니다. 설정할 것도, 만들 계정도 없어요.</p>
        <button className="btn primary" onClick={pick}>
          폴더 고르기
        </button>
      </div>

      {recents.length > 0 && (
        <div className="recent">
          <div className="rail-label">최근에 열었던 폴더</div>
          {recents.map((r) => (
            <button key={r.root} className="recent-row" onClick={() => onOpen(r.root)}>
              <span className="swatch" />
              <span>{r.name}</span>
              <span className="when">{ago(r.at)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ago(at: number) {
  const mins = Math.max(0, Math.floor(Date.now() / 1000 - at) / 60);
  if (mins < 1) return "방금";
  if (mins < 60) return `${Math.floor(mins)}분 전`;
  const hours = mins / 60;
  if (hours < 24) return `${Math.floor(hours)}시간 전`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "어제" : `${days}일 전`;
}
