//! "앱이 켜지는가"를 직접 확인한다.
//!
//! 타임라인의 ✅/❌는 이 앱의 유일한 진짜 차별점인데, 사용자가 직접 눌러야
//! 하는 동안에는 아무도 누르지 않는다. 그래서 프로젝트 종류를 알아보고
//! 실제로 한 번 돌려 본다.
//!
//! 판정은 세 갈래다. 성공/실패만이 아니라 **"판단할 수 없음"**을 분명히
//! 구분한다. 확인할 방법이 없는데 실패로 적으면 사용자를 엉뚱한 시점으로
//! 되돌리게 만든다.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::notes::{self, Meta};
use crate::repo::Project;
use crate::timeline::{self, Health};

/// 이보다 오래 걸리면 판단을 포기한다. 빌드가 느린 프로젝트도 있다.
const TIMEOUT: Duration = Duration::from_secs(180);
/// 실패 이유로 사용자에게 보여줄 최대 길이.
const DETAIL_LIMIT: usize = 1200;

/// 이 프로젝트를 어떻게 확인할지.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    /// 사용자에게 보여줄 이름. "앱 빌드", "타입 검사" 같은 말.
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub health: Health,
    /// 무엇으로 확인했는지. 판단 불가일 때는 왜 못 했는지.
    pub how: String,
    /// 실패했을 때 사용자에게 보여줄 이유.
    pub detail: Option<String>,
}

impl Outcome {
    fn unknown(how: impl Into<String>) -> Self {
        Self {
            health: Health::Unknown,
            how: how.into(),
            detail: None,
        }
    }
}

/// 프로젝트 종류를 알아보고 확인 방법을 고른다. 없으면 판단하지 않는다.
pub fn detect(root: &Path) -> std::result::Result<Probe, String> {
    if root.join("package.json").is_file() {
        return node_probe(root);
    }
    if root.join("Cargo.toml").is_file() {
        return Ok(Probe {
            label: "빌드 검사".into(),
            program: "cargo".into(),
            args: vec!["check".into(), "--quiet".into()],
        });
    }
    if root.join("pyproject.toml").is_file() || has_ext(root, "py") {
        return Ok(Probe {
            label: "문법 검사".into(),
            program: "python3".into(),
            args: vec!["-m".into(), "compileall".into(), "-q".into(), ".".into()],
        });
    }
    Err("이 프로젝트는 확인할 방법을 아직 몰라요".into())
}

fn node_probe(root: &Path) -> std::result::Result<Probe, String> {
    // node_modules가 없으면 어떤 명령도 진짜 실패인지 알 수 없다.
    if !root.join("node_modules").is_dir() {
        return Err("먼저 의존성을 설치해야 확인할 수 있어요".into());
    }

    let body = std::fs::read_to_string(root.join("package.json"))
        .map_err(|_| "package.json을 읽을 수 없어요".to_string())?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "package.json 형식이 깨졌어요".to_string())?;
    let scripts = json.get("scripts").and_then(|s| s.as_object());

    let runner = runner_for(root);

    // build가 가장 넓게 잡는다. 타입 오류, 빠진 파일, 문법 오류가 여기서 걸린다.
    for name in ["build", "typecheck", "type-check", "tsc"] {
        if scripts.map(|s| s.contains_key(name)).unwrap_or(false) {
            return Ok(Probe {
                label: if name == "build" {
                    "앱 빌드".into()
                } else {
                    "타입 검사".into()
                },
                program: runner.into(),
                args: vec!["run".into(), name.into()],
            });
        }
    }

    if root.join("tsconfig.json").is_file() {
        return Ok(Probe {
            label: "타입 검사".into(),
            program: runner.into(),
            args: vec![
                "exec".into(),
                "tsc".into(),
                "--noEmit".into(),
            ],
        });
    }

    Err("확인에 쓸 만한 명령을 찾지 못했어요".into())
}

fn runner_for(root: &Path) -> &'static str {
    if root.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if root.join("yarn.lock").is_file() {
        "yarn"
    } else {
        "npm"
    }
}

fn has_ext(root: &Path, ext: &str) -> bool {
    std::fs::read_dir(root)
        .map(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .map(|x| x == ext)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// 실제로 돌려 본다.
pub fn run(root: &Path, probe: &Probe) -> Outcome {
    let started = Instant::now();
    let spawned = Command::new(&probe.program)
        .args(&probe.args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match spawned {
        Ok(c) => c,
        // 러너가 안 깔려 있으면 실패가 아니라 판단 불가다.
        Err(_) => {
            return Outcome::unknown(format!("{}을(를) 실행할 수 없어요", probe.program));
        }
    };

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => return Outcome::unknown("확인하는 중에 문제가 생겼어요"),
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            return Outcome::unknown(format!("{}이(가) 너무 오래 걸려요", probe.label));
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    let Ok(out) = child.wait_with_output() else {
        return Outcome::unknown("확인 결과를 읽지 못했어요");
    };

    if out.status.success() {
        return Outcome {
            health: Health::Ok,
            how: probe.label.clone(),
            detail: None,
        };
    }

    let mut text = String::from_utf8_lossy(&out.stderr).to_string();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&out.stdout).to_string();
    }
    Outcome {
        health: Health::Broken,
        how: probe.label.clone(),
        detail: Some(first_error(&text)),
    }
}

/// 출력 전체는 사용자에게 쓸모가 없다. 오류가 처음 나온 근처만 남긴다.
fn first_error(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();

    let anchor = lines.iter().position(|l| {
        let low = l.to_lowercase();
        low.contains("error")
            || low.contains("failed")
            || low.contains("cannot")
            || low.contains("traceback")
    });

    let picked: Vec<&str> = match anchor {
        Some(i) => lines[i..].iter().take(12).copied().collect(),
        // 오류 표시가 없으면 끝부분이 보통 이유를 담고 있다.
        None => lines.iter().rev().take(12).rev().copied().collect(),
    };

    let joined = picked.join("\n");
    if joined.chars().count() > DETAIL_LIMIT {
        joined.chars().take(DETAIL_LIMIT).collect::<String>() + "…"
    } else {
        joined
    }
}

/// 지금 폴더 상태를 확인하고 그 결과를 세이브 포인트에 적는다.
///
/// 확인은 항상 **작업 폴더의 지금 내용**을 대상으로 한다. 그래서 `id`는
/// 보통 HEAD여야 한다. 과거 시점을 확인하려면 먼저 그리로 되돌려야 한다.
pub fn check_and_mark(project: &Project, id: &str) -> Result<Outcome> {
    let probe = match detect(&project.root) {
        Ok(p) => p,
        Err(why) => {
            let outcome = Outcome::unknown(why);
            write(project, id, &outcome)?;
            return Ok(outcome);
        }
    };

    let outcome = run(&project.root, &probe);
    write(project, id, &outcome)?;
    Ok(outcome)
}

fn write(project: &Project, id: &str, outcome: &Outcome) -> Result<()> {
    let commit = timeline::resolve(project, id)?;
    let health = match outcome.health {
        Health::Ok => "ok",
        Health::Broken => "broken",
        Health::Unknown => "unknown",
    };
    notes::merge(
        project,
        commit.id(),
        Meta {
            health: Some(health.to_string()),
            checked_by: Some(outcome.how.clone()),
            broke_because: outcome.detail.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
