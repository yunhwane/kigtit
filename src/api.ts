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
  /** 무엇으로 확인했는지 ("앱 빌드", "타입 검사"). 미확인이면 null. */
  checked_by: string | null;
  /** 안 켜졌을 때 그 이유. 사용자에게 그대로 보여준다. */
  broke_because: string | null;
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

export type Readiness =
  | { state: "ready"; account: string }
  | { state: "notSignedIn" }
  | { state: "noTool" };

export interface BackupStatus {
  readiness: Readiness;
  remote: string | null;
  /** 아직 백업되지 않은 세이브 포인트 수. */
  unbacked: number;
  branch: string;
}

export interface BackupDone {
  remote: string;
  backed_up: number;
  created: boolean;
}

export type Side = "mine" | "theirs";

export interface Conflict {
  path: string;
  mine_deleted: boolean;
  theirs_deleted: boolean;
}

export type SyncOutcome =
  | { kind: "upToDate" }
  | { kind: "pulled"; count: number }
  | { kind: "merged"; count: number }
  | { kind: "needsChoice"; conflicts: Conflict[] }
  | { kind: "noRemote" };

export interface Explanation {
  path: string;
  /** 내 컴퓨터에서 이 파일에 무엇을 했는지. */
  mine: string;
  /** GitHub 쪽에서 무엇을 했는지. */
  theirs: string;
}

export interface Outcome {
  health: Health;
  /** 무엇으로 확인했는지. 판단 불가일 때는 왜 못 했는지. */
  how: string;
  detail: string | null;
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
  checkHealth: () => invoke<Outcome>("check_health"),
  healthProbe: () => invoke<string>("health_probe"),
  backupStatus: () => invoke<BackupStatus>("backup_status"),
  backupGuard: () => invoke<Finding[]>("backup_guard"),
  backupRun: (private_: boolean) => invoke<BackupDone>("backup_run", { private: private_ }),
  sync: () => invoke<SyncOutcome>("sync_now"),
  syncResolve: (choices: [string, Side][]) => invoke<SavePoint>("sync_resolve", { choices }),
  syncExplain: (path: string) => invoke<Explanation>("sync_explain", { path }),
};

/** 자동 저장이 알려오는 것들. 창이 살아 있는 동안 계속 들어온다. */
export function onWatch(handlers: {
  changed?: (files: number) => void;
  saved?: (point: SavePoint) => void;
  blocked?: (findings: Finding[]) => void;
  summarized?: (id: string, summary: Summary) => void;
  checked?: (id: string, outcome: Outcome) => void;
  resuming?: (count: number) => void;
}) {
  const offs = [
    listen<number>("kigtit:changed", (e) => handlers.changed?.(e.payload)),
    listen<SavePoint>("kigtit:saved", (e) => handlers.saved?.(e.payload)),
    listen<Finding[]>("kigtit:blocked", (e) => handlers.blocked?.(e.payload)),
    listen<[string, Summary]>("kigtit:summarized", (e) =>
      handlers.summarized?.(e.payload[0], e.payload[1]),
    ),
    listen<[string, Outcome]>("kigtit:checked", (e) =>
      handlers.checked?.(e.payload[0], e.payload[1]),
    ),
    listen<number>("kigtit:resuming", (e) => handlers.resuming?.(e.payload)),
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
