import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Health = "ok" | "broken" | "unknown";
export type SaveKind = "auto" | "manual" | "restore" | "start";

export interface FileChange {
  path: string;
  kind: string;
  added: number;
  removed: number;
}

export interface SavePoint {
  id: string;
  full_id: string;
  at: number;
  at_label: string;
  title: string;
  summary: string | null;
  kind: SaveKind;
  health: Health;
  files: FileChange[];
  /** AI 요약이 아직 도착하지 않았다 → 카드에 "요약 중…"을 띄운다. */
  pending_summary: boolean;
}

export interface Finding {
  risk: "secret" | "bigfile";
  path: string;
  line: number | null;
  message: string;
  masked: string | null;
  advice: string;
}

export interface ProjectInfo {
  root: string;
  name: string;
  agent: "claude" | "codex" | "rules";
  agent_label: string;
  has_history: boolean;
}

export interface View {
  points: SavePoint[];
  pending: FileChange[];
  last_healthy: string | null;
}

export interface Restored {
  created: SavePoint;
  target_id: string;
  target_title: string;
  snapshot_id: string | null;
}

export interface Recent {
  root: string;
  name: string;
  at: number;
}

export type SaveReply =
  | { kind: "saved"; point: SavePoint }
  | { kind: "noChanges" }
  | { kind: "blocked"; findings: Finding[] };

export interface Summary {
  title: string;
  summary: string;
  by: string;
}

export const api = {
  openProject: (path: string) => invoke<ProjectInfo>("open_project", { path }),
  recent: () => invoke<Recent[]>("recent"),
  launchFolder: () => invoke<string | null>("launch_folder"),
  view: (limit = 40) => invoke<View>("view", { limit }),
  save: (title?: string) => invoke<SaveReply>("save_now", { title: title ?? null }),
  restoreTo: (id: string) => invoke<Restored>("restore_to", { id }),
  undo: () => invoke<Restored>("undo"),
  check: () => invoke<Finding[]>("check"),
  exclude: (path: string) => invoke<void>("exclude", { path }),
  mark: (id: string, health: Health) => invoke<SavePoint>("mark", { id, health }),
  patch: (id: string) => invoke<string>("patch", { id }),
  summarize: (id: string) => invoke<Summary>("summarize", { id }),
};

/** 자동 저장이 알려오는 것들. 창이 살아 있는 동안 계속 들어온다. */
export function onWatch(handlers: {
  changed?: (files: number) => void;
  saved?: (point: SavePoint) => void;
  blocked?: (findings: Finding[]) => void;
  summarized?: (id: string, summary: Summary) => void;
}) {
  const offs = [
    listen<number>("kigtit:changed", (e) => handlers.changed?.(e.payload)),
    listen<SavePoint>("kigtit:saved", (e) => handlers.saved?.(e.payload)),
    listen<Finding[]>("kigtit:blocked", (e) => handlers.blocked?.(e.payload)),
    listen<[string, Summary]>("kigtit:summarized", (e) =>
      handlers.summarized?.(e.payload[0], e.payload[1]),
    ),
  ];
  return () => {
    offs.forEach((p) => p.then((off) => off()));
  };
}

export const HEALTH_LABEL: Record<Health, string> = {
  ok: "앱 잘 켜짐",
  broken: "여기서 앱이 안 켜졌어요",
  unknown: "확인 안 됨",
};

/** 색만으로 상태를 말하지 않는다. 도형을 같이 쓴다. */
export const HEALTH_GLYPH: Record<Health, string> = {
  ok: "●",
  broken: "■",
  unknown: "○",
};
