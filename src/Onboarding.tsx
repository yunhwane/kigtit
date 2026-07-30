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
    const path = await open({ directory: true, title: "Choose the folder you were working in" });
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
        <h2>Drop the folder you've been working in</h2>
        <p>We'll take care of the rest. No settings or account required.</p>
        <button className="btn primary" onClick={pick}>
          Choose folder
        </button>
      </div>

      {recents.length > 0 && (
        <div className="recent">
          <div className="rail-label">Recently opened folders</div>
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
  if (mins < 1) return "Just now";
  if (mins < 60) return `${Math.floor(mins)}m ago`;
  const hours = mins / 60;
  if (hours < 24) return `${Math.floor(hours)}h ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "Yesterday" : `${days}d ago`;
}
