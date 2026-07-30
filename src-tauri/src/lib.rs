//! Kigtit 데스크톱 앱의 백엔드.
//!
//! 로직은 전부 `kigtit-core`에 있고 여기서는 창과 프런트엔드를 잇기만 한다.
//! CLI와 앱이 같은 코어를 쓰므로 동작이 갈라지지 않는다.
//!
//! `git2::Repository`는 스레드 간에 넘길 수 없어서 상태로 들고 있지 않는다.
//! 명령마다 경로로 다시 여는데, 그 비용은 밀리초 단위다.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use kigtit_core::{
    Project, ai, backup, health, notes, restore, save, secrets, sync, timeline,
    watch::{self, Event as WatchEvent},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

// ── 상태 ─────────────────────────────────────────────────

#[derive(Default)]
struct App {
    root: Mutex<Option<PathBuf>>,
    /// 지금 돌고 있는 감시를 세우는 스위치. 프로젝트를 바꿀 때 내린다.
    watching: Mutex<Option<Arc<AtomicBool>>>,
}

impl App {
    fn root(&self) -> Result<PathBuf, String> {
        self.root
            .lock()
            .map_err(|_| "상태를 읽지 못했어요.".to_string())?
            .clone()
            .ok_or_else(|| "아직 열린 프로젝트가 없어요.".to_string())
    }

    fn project(&self) -> Result<Project, String> {
        Project::open(self.root()?).map_err(err)
    }
}

fn err(e: anyhow::Error) -> String {
    e.to_string()
}

// ── 프런트엔드에 넘기는 모양 ──────────────────────────────

#[derive(Serialize)]
struct ProjectInfo {
    root: String,
    name: String,
    /// 요약을 맡을 도구. "rules"면 AI CLI가 없다는 뜻.
    agent: &'static str,
    agent_label: &'static str,
    has_history: bool,
}

#[derive(Serialize)]
struct View {
    points: Vec<timeline::SavePoint>,
    pending: Vec<timeline::FileChange>,
    /// 앱이 마지막으로 잘 켜졌던 시점. 되돌리기 확인창의 추천값.
    last_healthy: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum SaveReply {
    Saved { point: timeline::SavePoint },
    NoChanges,
    /// 비밀 키를 찾아 멈췄다. 유일하게 흐름을 끊는 경우.
    Blocked { findings: Vec<secrets::Finding> },
}

#[derive(Serialize, Deserialize, Default)]
struct Recents {
    items: Vec<Recent>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Recent {
    root: String,
    name: String,
    at: i64,
}

// ── 명령 ─────────────────────────────────────────────────

#[tauri::command]
fn open_project(path: String, app: AppHandle, state: State<App>) -> Result<ProjectInfo, String> {
    let project = Project::open(&path).map_err(err)?;
    let root = project.root.clone();
    let name = root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "프로젝트".into());

    let agent = ai::detect();
    let info = ProjectInfo {
        root: root.to_string_lossy().to_string(),
        name: name.clone(),
        agent: agent.as_str(),
        agent_label: agent.label(),
        has_history: project.has_history(),
    };

    *state.root.lock().map_err(|_| "상태 오류")? = Some(root.clone());
    remember(&app, &root, &name);
    start_watch(&app, root, &state);
    Ok(info)
}

#[tauri::command]
fn recent(app: AppHandle) -> Vec<Recent> {
    read_recents(&app).items
}

/// `kigtit` CLI가 폴더를 넘겨 앱을 띄웠다면 그 폴더. 첫 화면을 건너뛰는 데 쓴다.
#[tauri::command]
fn launch_folder() -> Option<String> {
    std::env::args()
        .nth(1)
        .filter(|a| !a.starts_with('-') && PathBuf::from(a).is_dir())
}

#[tauri::command]
fn view(limit: usize, state: State<App>) -> Result<View, String> {
    let project = state.project()?;
    Ok(View {
        points: timeline::list(&project, limit).map_err(err)?,
        pending: timeline::uncommitted(&project).map_err(err)?,
        last_healthy: restore::last_healthy(&project)
            .map_err(err)?
            .map(|sp| sp.id),
    })
}

#[tauri::command]
fn save_now(title: Option<String>, state: State<App>) -> Result<SaveReply, String> {
    let project = state.project()?;

    let findings = secrets::scan_pending(&project).unwrap_or_default();
    if findings.iter().any(|f| f.risk == secrets::Risk::Secret) {
        return Ok(SaveReply::Blocked { findings });
    }

    let kind = if title.is_some() {
        save::SaveKind::Manual
    } else {
        save::SaveKind::Auto
    };
    match save::save(&project, title.as_deref(), kind).map_err(err)? {
        save::SaveOutcome::Saved(point) => Ok(SaveReply::Saved { point }),
        save::SaveOutcome::NoChanges => Ok(SaveReply::NoChanges),
    }
}

#[tauri::command]
fn restore_to(id: String, state: State<App>) -> Result<restore::Restored, String> {
    let project = state.project()?;
    restore::restore_to(&project, &id).map_err(err)
}

#[tauri::command]
fn undo(state: State<App>) -> Result<restore::Restored, String> {
    let project = state.project()?;
    restore::undo(&project).map_err(err)
}

#[tauri::command]
fn check(state: State<App>) -> Result<Vec<secrets::Finding>, String> {
    let project = state.project()?;
    secrets::scan_pending(&project).map_err(err)
}

/// 위험한 파일을 백업에서 뺀다.
#[tauri::command]
fn exclude(path: String, state: State<App>) -> Result<(), String> {
    let project = state.project()?;
    project.exclude(&path).map_err(err)
}

#[tauri::command]
fn mark(id: String, health: String, state: State<App>) -> Result<timeline::SavePoint, String> {
    let project = state.project()?;
    let commit = timeline::resolve(&project, &id).map_err(err)?;
    let oid = commit.id();
    notes::merge(
        &project,
        oid,
        notes::Meta {
            health: Some(health),
            ..Default::default()
        },
    )
    .map_err(err)?;
    timeline::find(&project, &id).map_err(err)
}

/// 코드로 보기.
#[tauri::command]
fn patch(id: String, state: State<App>) -> Result<String, String> {
    let project = state.project()?;
    let commit = timeline::resolve(&project, &id).map_err(err)?;
    ai::patch_for(&project, &commit).map_err(err)
}

/// 요약이 빠진 세이브 포인트를 채운다. 8초쯤 걸리므로 프런트엔드에서 기다린다.
#[tauri::command]
fn summarize(id: String, state: State<App>) -> Result<ai::Summary, String> {
    let project = state.project()?;
    ai::summarize_save_point(&project, &id, ai::detect()).map_err(err)
}

/// 앱이 켜지는지 지금 확인한다. 빌드를 돌리므로 오래 걸릴 수 있다.
#[tauri::command]
fn check_health(state: State<App>) -> Result<health::Outcome, String> {
    let project = state.project()?;
    health::check_and_mark(&project, "HEAD").map_err(err)
}

// ── GitHub 백업 ───────────────────────────────────────────

/// 백업 상태. 네트워크를 타지 않으므로 화면을 그릴 때 바로 부른다.
#[tauri::command]
fn backup_status(state: State<App>) -> Result<backup::Status, String> {
    let project = state.project()?;
    Ok(backup::status(&project))
}

/// 올리기 전에 비밀 키를 검사한다. **이미 담긴 파일까지** 본다.
#[tauri::command]
fn backup_guard(state: State<App>) -> Result<Vec<secrets::Finding>, String> {
    let project = state.project()?;
    backup::guard(&project).map_err(err)
}

/// GitHub에 올린다. `private`는 프런트엔드가 반드시 정해서 보낸다.
#[tauri::command]
fn backup_run(private: bool, state: State<App>) -> Result<backup::Done, String> {
    let project = state.project()?;
    backup::run(&project, private).map_err(err)
}

// ── 맞추기 ────────────────────────────────────────────────

/// GitHub 쪽 변경을 가져온다. 겹치면 작업 폴더를 건드리지 않고 목록만 준다.
#[tauri::command]
fn sync_now(state: State<App>) -> Result<sync::Outcome, String> {
    let project = state.project()?;
    sync::sync(&project).map_err(err)
}

/// 파일마다 어느 쪽을 남길지 정해서 한 번에 적용한다.
#[tauri::command]
fn sync_resolve(
    choices: Vec<(String, sync::Side)>,
    state: State<App>,
) -> Result<timeline::SavePoint, String> {
    let project = state.project()?;
    sync::resolve(&project, &choices).map_err(err)
}

/// 겹친 파일에서 양쪽이 뭘 하려 했는지 사람 말로 설명한다.
///
/// 충돌 화면의 핵심이다. 코드를 못 읽는 사람은 `<<<<<<< HEAD`를 봐도
/// 고를 수가 없다. 무엇을 하려던 변경인지 알아야 고를 수 있다.
#[tauri::command]
fn sync_explain(path: String, state: State<App>) -> Result<sync::Explanation, String> {
    let project = state.project()?;
    sync::explain(&project, &path, ai::detect()).map_err(err)
}

/// 이 프로젝트를 어떻게 확인하는지. 확인할 방법이 없으면 그 이유.
#[tauri::command]
fn health_probe(state: State<App>) -> Result<String, String> {
    let project = state.project()?;
    Ok(match health::detect(&project.root) {
        Ok(probe) => probe.label,
        Err(why) => why,
    })
}

// ── 자동 저장 ─────────────────────────────────────────────

/// 새 프로젝트를 열 때마다 이전 감시를 내리고 새로 띄운다.
fn start_watch(app: &AppHandle, root: PathBuf, state: &State<App>) {
    if let Ok(mut slot) = state.watching.lock() {
        if let Some(old) = slot.take() {
            old.store(true, Ordering::Relaxed);
        }
        let stop = Arc::new(AtomicBool::new(false));
        *slot = Some(stop.clone());

        let app = app.clone();
        std::thread::spawn(move || {
            let _ = watch::watch(&root, watch::DEFAULT_IDLE, stop, |event| match event {
                WatchEvent::Changed { files } => {
                    let _ = app.emit("kigtit:changed", files);
                }
                WatchEvent::Saved(point) => {
                    let _ = app.emit("kigtit:saved", point);
                }
                WatchEvent::Blocked(findings) => {
                    let _ = app.emit("kigtit:blocked", findings);
                }
                WatchEvent::Summarized { id, summary } => {
                    let _ = app.emit("kigtit:summarized", (id, summary));
                }
                WatchEvent::Checked { id, outcome } => {
                    let _ = app.emit("kigtit:checked", (id, outcome));
                }
                WatchEvent::Resuming { count } => {
                    let _ = app.emit("kigtit:resuming", count);
                }
            });
        });
    }
}

// ── 최근 프로젝트 ─────────────────────────────────────────

fn recents_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("recent.json"))
}

fn read_recents(app: &AppHandle) -> Recents {
    recents_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn remember(app: &AppHandle, root: &PathBuf, name: &str) {
    let root = root.to_string_lossy().to_string();
    let mut recents = read_recents(app);
    recents.items.retain(|r| r.root != root);
    recents.items.insert(
        0,
        Recent {
            root,
            name: name.to_string(),
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        },
    );
    recents.items.truncate(8);

    if let (Some(path), Ok(body)) = (recents_path(app), serde_json::to_string(&recents)) {
        let _ = std::fs::write(path, body);
    }
}

// ── 시작 ─────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App::default())
        .invoke_handler(tauri::generate_handler![
            open_project,
            recent,
            launch_folder,
            view,
            save_now,
            restore_to,
            undo,
            check,
            exclude,
            mark,
            patch,
            summarize,
            check_health,
            health_probe,
            backup_status,
            backup_guard,
            backup_run,
            sync_now,
            sync_resolve,
            sync_explain,
        ])
        .run(tauri::generate_context!())
        .expect("Kigtit을 시작하지 못했어요.");
}
