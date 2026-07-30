//! 자동 저장.
//!
//! 사용자는 "저장해야 한다"는 사실을 배우지 않는다. 파일이 바뀌고 잠시
//! 조용해지면 세이브 포인트가 알아서 생긴다. AI가 파일을 쏟아내는 동안은
//! 계속 조용해지기를 기다리므로, 시도 한 번이 세이브 포인트 하나가 된다.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use anyhow::Context;
use notify::{EventKind, RecursiveMode, Watcher};

use crate::Result;
use crate::ai;
use crate::health;
use crate::repo::Project;
use crate::save::{self, SaveKind, SaveOutcome};
use crate::secrets::{self, Finding, Risk};
use crate::timeline::SavePoint;

/// 자동 저장이 일어나는 동안 바깥에 알려줄 일들.
pub enum Event {
    /// 파일이 바뀌었다. 아직 담지는 않았다.
    Changed { files: usize },
    /// 새 세이브 포인트가 생겼다.
    Saved(SavePoint),
    /// 비밀 키를 찾아 저장을 멈췄다. 유일하게 흐름을 끊는 순간.
    Blocked(Vec<Finding>),
    /// AI 요약이 뒤늦게 도착했다.
    Summarized { id: String, summary: ai::Summary },
    /// 앱이 켜지는지 확인해 봤다.
    Checked {
        id: String,
        outcome: health::Outcome,
    },
    /// 꺼져 있는 동안 놓친 요약을 이어서 채우기 시작했다.
    Resuming { count: usize },
}

/// 다시 켤 때 요약을 이어 채울 범위. 최근 이만큼만 훑는다.
const RESUME_SCAN: usize = 40;
/// 한 번에 이어 채울 최대 개수. 요약 하나에 8초쯤 걸리므로 무한정 돌리지 않는다.
const RESUME_LIMIT: usize = 8;

/// 파일 변경 후 이만큼 조용하면 담는다.
pub const DEFAULT_IDLE: Duration = Duration::from_secs(3);

/// 감시할 필요가 없는 경로. 여기서 걸러야 AI가 만드는 빌드 산출물에 휘둘리지 않는다.
fn is_noise(rel: &Path) -> bool {
    const SKIP: &[&str] = &[
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        ".next",
        ".nuxt",
        ".turbo",
        ".cache",
        "__pycache__",
        ".venv",
        "venv",
        "coverage",
        ".pnpm-store",
    ];
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| SKIP.contains(&s))
            .unwrap_or(false)
    })
}

/// 폴더를 계속 지켜보며 조용해질 때마다 담는다.
///
/// `stop`을 `true`로 만들면 다음 한 바퀴에서 끝난다. 앱에서 다른 프로젝트를
/// 열 때 이전 감시를 정리하는 데 쓴다.
pub fn watch(
    root: impl AsRef<Path>,
    idle: Duration,
    stop: Arc<AtomicBool>,
    mut on_event: impl FnMut(Event),
) -> Result<()> {
    let root = root.as_ref().to_path_buf();
    let project = Project::open(&root)?;
    let root = project.root.clone();

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .context("Could not start watching the folder.")?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .with_context(|| format!("Could not watch {}.", root.display()))?;

    // 요약은 8초쯤 걸린다. 별 스레드에 맡기고 결과만 받아서, 그 사이에도
    // 파일 변경을 계속 받아들인다.
    let (sum_tx, sum_rx) = mpsc::channel::<(String, ai::Summary)>();
    let (chk_tx, chk_res) = spawn_checker(root.clone());

    // 앱을 강제로 끄면 진행 중이던 요약이 같이 죽는다. 대기열을 따로 파일로
    // 두지는 않는다 — **요약이 없는 세이브 포인트 자체가 대기열**이고, 그건
    // 이미 git에 남아 있다. 파일로 두면 손상되거나 실제 상태와 어긋날 뿐이다.
    let missed = resume_pending(root.clone(), sum_tx.clone());
    if missed > 0 {
        on_event(Event::Resuming { count: missed });
    }

    let mut dirty_since: Option<Instant> = None;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        while let Ok((id, summary)) = sum_rx.try_recv() {
            on_event(Event::Summarized { id, summary });
        }
        while let Ok((id, outcome)) = chk_res.try_recv() {
            on_event(Event::Checked { id, outcome });
        }

        match rx.recv_timeout(Duration::from_millis(400)) {
            Ok(Ok(event)) => {
                if !matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    continue;
                }
                let relevant = event.paths.iter().any(|p| {
                    let rel = p.strip_prefix(&root).unwrap_or(p);
                    !is_noise(rel)
                });
                if !relevant {
                    continue;
                }
                if dirty_since.is_none() {
                    let files = crate::timeline::uncommitted(&project)
                        .map(|f| f.len())
                        .unwrap_or(0);
                    on_event(Event::Changed { files });
                }
                dirty_since = Some(Instant::now());
            }
            Ok(Err(_)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let ready = dirty_since.map(|t| t.elapsed() >= idle).unwrap_or(false);
        if !ready {
            continue;
        }
        dirty_since = None;

        let findings = secrets::scan_pending(&project).unwrap_or_default();
        if findings.iter().any(|f| f.risk == Risk::Secret) {
            on_event(Event::Blocked(findings));
            continue;
        }

        match save::save(&project, None, SaveKind::Auto) {
            Ok(SaveOutcome::Saved(sp)) => {
                let id = sp.full_id.clone();
                if sp.pending_summary {
                    summarize_detached(root.clone(), id.clone(), sum_tx.clone());
                }
                let _ = chk_tx.send(id);
                on_event(Event::Saved(sp));
            }
            Ok(SaveOutcome::NoChanges) => {}
            Err(_) => {}
        }
    }

    Ok(())
}

/// 꺼져 있는 동안 놓친 요약을 이어서 채운다. 채울 개수를 바로 돌려준다.
///
/// 요약 자체는 별 스레드에서 이어지므로 감시 시작을 붙잡지 않는다.
fn resume_pending(root: PathBuf, tx: mpsc::Sender<(String, ai::Summary)>) -> usize {
    let Ok(project) = Project::open(&root) else {
        return 0;
    };
    let agent = ai::detect();
    if agent == ai::Agent::Rules {
        // 규칙 기반 요약은 언제든 즉시 만들 수 있으니 굳이 몰아서 채우지 않는다.
        return 0;
    }

    // 최근 것이 사용자에게 더 쓸모 있으므로 최근 순으로 고르고,
    // 처리는 오래된 것부터 해서 타임라인이 아래에서 위로 채워지게 한다.
    let mut pending: Vec<String> = crate::timeline::list(&project, RESUME_SCAN)
        .unwrap_or_default()
        .into_iter()
        .filter(|sp| sp.pending_summary)
        .map(|sp| sp.full_id)
        .take(RESUME_LIMIT)
        .collect();
    pending.reverse();

    let count = pending.len();
    if count == 0 {
        return 0;
    }

    std::thread::spawn(move || {
        let Ok(project) = Project::open(&root) else {
            return;
        };
        for id in pending {
            if let Ok(summary) = ai::summarize_save_point(&project, &id, agent) {
                let _ = tx.send((id, summary));
            }
        }
    });
    count
}

/// "앱이 켜지는가"를 확인하는 스레드 하나.
///
/// 빌드는 30초씩 걸릴 수 있어서 저장마다 새로 띄우면 쌓인다. 그래서 하나만
/// 두고, 밀린 요청은 **가장 새것만** 확인한다. 판정은 늦더라도 수렴한다.
fn spawn_checker(
    root: PathBuf,
) -> (
    mpsc::Sender<String>,
    mpsc::Receiver<(String, health::Outcome)>,
) {
    let (tx, rx) = mpsc::channel::<String>();
    let (res_tx, res_rx) = mpsc::channel();

    std::thread::spawn(move || {
        while let Ok(mut id) = rx.recv() {
            while let Ok(newer) = rx.try_recv() {
                id = newer;
            }
            let Ok(project) = Project::open(&root) else {
                continue;
            };
            // 확인은 작업 폴더의 지금 내용을 대상으로 한다. 그 사이 새
            // 세이브 포인트가 생겼다면 이 결과는 그 시점의 것이 아니다.
            // 엉뚱한 시점에 ❌를 붙이지 않도록 버린다 — 새것이 곧 큐에 온다.
            let at_head = project
                .head_commit()
                .map(|c| c.id().to_string() == id)
                .unwrap_or(false);
            if !at_head {
                continue;
            }
            if let Ok(outcome) = health::check_and_mark(&project, &id) {
                let _ = res_tx.send((id, outcome));
            }
        }
    });

    (tx, res_rx)
}

/// `git2::Repository`는 스레드 간에 넘길 수 없으므로 경로만 넘기고
/// 새 스레드에서 다시 연다.
fn summarize_detached(root: PathBuf, id: String, tx: mpsc::Sender<(String, ai::Summary)>) {
    std::thread::spawn(move || {
        let Ok(project) = Project::open(&root) else {
            return;
        };
        if let Ok(summary) = ai::summarize_save_point(&project, &id, ai::detect()) {
            let _ = tx.send((id, summary));
        }
    });
}
