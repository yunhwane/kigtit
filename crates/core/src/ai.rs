//! 사람 말 요약.
//!
//! **API 키를 받지 않는다.** 타겟 사용자는 이미 AI CLI를 깔아서 로그인해 둔
//! 바이브 코더다. PATH에서 그걸 찾아 그대로 빌려 쓴다. 설정 화면이 없고,
//! 요금이 따로 붙지 않고, 키가 유출될 표면도 없다.
//!
//! 아무 CLI도 없으면 파일 목록으로 기계적 요약을 만든다. 기능이 죽지는 않는다.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::notes::{self, Meta};
use crate::repo::Project;
use crate::timeline::{self, FileChange};
use crate::tools;

/// diff를 이 길이로 잘라 넘긴다. 큰 변경도 요약 품질이 유지되는 선.
const DIFF_LIMIT: usize = 12_000;
const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
    /// AI CLI가 없을 때의 기계적 요약.
    Rules,
}

impl Agent {
    pub fn as_str(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Rules => "rules",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Rules => "요약 도구 없음",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub title: String,
    pub summary: String,
    pub by: String,
}

/// PATH에 있는 AI CLI를 찾는다. 앞쪽이 우선순위.
pub fn detect() -> Agent {
    for agent in [Agent::Claude, Agent::Codex] {
        if tools::exists(agent.as_str()) {
            return agent;
        }
    }
    Agent::Rules
}

const PROMPT: &str = "\
당신은 코딩을 배운 적 없는 사람에게 방금 일어난 변경을 설명합니다.
입력은 git diff입니다.

규칙:
- 반드시 한국어로 씁니다.
- 코드 용어를 쓰지 않습니다. 금지: 함수, 변수, 컴포넌트, 리팩터링, 커밋, props, state, import.
- 화면에서 무엇이 달라지는지, 사용자가 무엇을 다르게 겪는지를 씁니다.
- 파일 이름은 필요할 때만 씁니다.
- 추측하지 않습니다. diff에 없는 내용을 만들지 않습니다.

아래 JSON만 출력합니다. 다른 말은 한 글자도 붙이지 마세요.
{\"title\": \"12자 이내 제목\", \"summary\": \"2~3문장 설명\"}";

/// 세이브 포인트 하나를 요약해 `refs/notes/kigtit`에 붙인다.
///
/// 8초쯤 걸리므로 호출하는 쪽에서 백그라운드로 돌린다. 커밋 해시는 바뀌지 않는다.
pub fn summarize_save_point(project: &Project, id: &str, agent: Agent) -> Result<Summary> {
    let commit = timeline::resolve(project, id)?;
    let oid = commit.id();
    let files = timeline::describe(project, &commit)?.files;
    let diff = patch_for(project, &commit)?;

    let summary = match agent {
        Agent::Rules => from_rules(&files),
        _ => run_agent(agent, &diff).unwrap_or_else(|_| from_rules(&files)),
    };

    notes::merge(
        project,
        oid,
        Meta {
            title: Some(summary.title.clone()),
            summary: Some(summary.summary.clone()),
            by: Some(summary.by.clone()),
            ..Default::default()
        },
    )?;

    Ok(summary)
}

/// 아직 요약이 없는 세이브 포인트를 오래된 것부터 채운다.
pub fn backfill(project: &Project, agent: Agent, limit: usize) -> Result<usize> {
    let pending: Vec<String> = timeline::list(project, limit)?
        .into_iter()
        .filter(|sp| sp.pending_summary)
        .map(|sp| sp.full_id)
        .rev()
        .collect();

    let mut done = 0;
    for id in pending {
        if summarize_save_point(project, &id, agent).is_ok() {
            done += 1;
        }
    }
    Ok(done)
}

/// 한 덩이의 변경을 한두 문장으로. 충돌 화면이 양쪽을 설명할 때 쓴다.
pub fn describe_change(patch: &str, agent: Agent) -> String {
    if agent == Agent::Rules {
        let (added, removed) = count_lines(patch);
        return format!(
            "{added}줄이 늘고 {removed}줄이 줄었어요. 자세한 설명을 보려면 Claude Code나 Codex를 설치해 주세요."
        );
    }
    match run_agent(agent, patch) {
        Ok(s) => s.summary,
        Err(_) => {
            let (added, removed) = count_lines(patch);
            format!("{added}줄이 늘고 {removed}줄이 줄었어요.")
        }
    }
}

fn count_lines(patch: &str) -> (usize, usize) {
    let added = patch
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .count();
    let removed = patch
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .count();
    (added, removed)
}

fn run_agent(agent: Agent, diff: &str) -> Result<Summary> {
    // PATH에 없을 수도 있으므로 찾아낸 절대 경로로 실행한다.
    let program = tools::resolve(agent.as_str())
        .ok_or_else(|| anyhow!("{}을(를) 찾지 못했어요.", agent.label()))?;
    let mut cmd = Command::new(program);
    match agent {
        Agent::Claude => {
            // 요약은 순수 텍스트 변환이다. 도구 접근을 완전히 끊는다.
            cmd.args(["-p", PROMPT, "--model", "haiku", "--allowed-tools", ""]);
        }
        Agent::Codex => {
            cmd.args([
                "exec",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                PROMPT,
            ]);
        }
        Agent::Rules => return Err(anyhow!("규칙 기반은 CLI를 쓰지 않아요.")),
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("{}을(를) 실행하지 못했어요.", agent.label()))?;

    let head: String = diff.chars().take(DIFF_LIMIT).collect();
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("입력을 넘기지 못했어요."))?
        .write_all(head.as_bytes())?;

    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            return Err(anyhow!("{} 응답이 너무 늦어요.", agent.label()));
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    let out = child.wait_with_output()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    parse(&text, agent)
}

/// 모델이 JSON 앞뒤에 말을 붙이는 경우가 있어 첫 객체만 꺼낸다.
fn parse(text: &str, agent: Agent) -> Result<Summary> {
    #[derive(Deserialize)]
    struct Raw {
        title: String,
        summary: String,
    }

    let start = text.find('{');
    let end = text.rfind('}');
    let raw: Raw = match (start, end) {
        (Some(s), Some(e)) if e > s => {
            serde_json::from_str(&text[s..=e]).map_err(|_| anyhow!("요약을 읽을 수 없어요."))?
        }
        _ => return Err(anyhow!("요약이 비어 있어요.")),
    };

    let title = raw.title.trim();
    let summary = raw.summary.trim();
    if title.is_empty() || summary.is_empty() {
        return Err(anyhow!("요약이 비어 있어요."));
    }

    Ok(Summary {
        title: title.to_string(),
        summary: summary.to_string(),
        by: agent.as_str().to_string(),
    })
}

/// AI CLI가 없을 때. 파일 목록만으로 만들 수 있는 만큼만 말한다.
fn from_rules(files: &[FileChange]) -> Summary {
    if files.is_empty() {
        return Summary {
            title: "변경 없음".into(),
            summary: "바뀐 파일이 없어요.".into(),
            by: Agent::Rules.as_str().into(),
        };
    }

    let added: usize = files.iter().map(|f| f.added).sum();
    let removed: usize = files.iter().map(|f| f.removed).sum();
    let new_files = files.iter().filter(|f| f.kind == "새 파일").count();

    let title = if new_files == files.len() {
        format!("새 파일 {}개 추가", new_files)
    } else {
        format!("파일 {}개 수정", files.len())
    };

    let names: Vec<&str> = files.iter().take(3).map(|f| f.path.as_str()).collect();
    let more = files.len().saturating_sub(names.len());
    let listed = if more > 0 {
        format!("{} 외 {}개", names.join(", "), more)
    } else {
        names.join(", ")
    };

    Summary {
        title,
        summary: format!(
            "{listed}가 바뀌었어요. 총 {added}줄이 늘고 {removed}줄이 줄었습니다. \
             자세한 설명을 보려면 Claude Code나 Codex를 설치해 주세요."
        ),
        by: Agent::Rules.as_str().into(),
    }
}

/// 커밋 하나의 patch 텍스트.
pub fn patch_for(project: &Project, commit: &git2::Commit<'_>) -> Result<String> {
    let tree = commit.tree()?;
    let parent = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let diff = project
        .repo
        .diff_tree_to_tree(parent.as_ref(), Some(&tree), None)?;

    let mut out = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        match line.origin() {
            '+' | '-' | ' ' => out.push(line.origin()),
            _ => {}
        }
        out.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;
    Ok(out)
}
