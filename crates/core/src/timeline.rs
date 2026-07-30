use std::collections::HashMap;

use chrono::{DateTime, Local, TimeZone};
use git2::{Commit, Delta, DiffOptions, Oid};
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::notes;
use crate::repo::Project;
use crate::save::SaveKind;

/// 이 시점에서 앱이 켜졌는가. 타임라인에서 색과 도형으로 같이 표시한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    /// 앱이 잘 켜짐
    Ok,
    /// 이 시점부터 앱이 안 켜졌음
    Broken,
    /// 아직 확인하지 않음
    Unknown,
}

impl Health {
    pub fn label(self) -> &'static str {
        match self {
            Health::Ok => "앱 잘 켜짐",
            Health::Broken => "여기서 앱이 안 켜졌어요",
            Health::Unknown => "확인 안 됨",
        }
    }

    /// 색맹 사용자와 흑백 출력에서도 구분되도록 도형을 같이 쓴다.
    pub fn glyph(self) -> &'static str {
        match self {
            Health::Ok => "●",
            Health::Broken => "■",
            Health::Unknown => "○",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    /// "새 파일" | "수정" | "삭제" | "이름 변경"
    pub kind: String,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavePoint {
    pub id: String,
    pub full_id: String,
    /// 유닉스 초. 프런트엔드에서 다시 포맷한다.
    pub at: i64,
    pub at_label: String,
    /// 화면에 보여줄 제목. AI 요약이 있으면 그것을, 없으면 커밋 첫 줄.
    pub title: String,
    pub summary: Option<String>,
    pub kind: SaveKind,
    pub health: Health,
    /// 무엇으로 확인했는지 ("앱 빌드", "타입 검사"). 미확인이면 None.
    pub checked_by: Option<String>,
    /// 안 켜졌을 때 그 이유. 사용자에게 그대로 보여준다.
    pub broke_because: Option<String>,
    pub files: Vec<FileChange>,
    /// 아직 AI 요약이 도착하지 않았다 → 카드에 "요약 중…"을 띄운다.
    pub pending_summary: bool,
}

/// 최근 세이브 포인트를 새것부터 최대 `limit`개.
pub fn list(project: &Project, limit: usize) -> Result<Vec<SavePoint>> {
    if !project.has_history() {
        return Ok(Vec::new());
    }

    let mut walk = project.repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TIME)?;

    let mut out = Vec::new();
    for oid in walk.take(limit) {
        let commit = project.repo.find_commit(oid?)?;
        out.push(describe(project, &commit)?);
    }
    Ok(out)
}

pub fn find(project: &Project, id: &str) -> Result<SavePoint> {
    let commit = resolve(project, id)?;
    describe(project, &commit)
}

/// 짧은 id, 전체 해시, `main~2` 같은 표현을 모두 받는다.
pub fn resolve<'a>(project: &'a Project, id: &str) -> Result<Commit<'a>> {
    let obj = project
        .repo
        .revparse_single(id)
        .map_err(|_| crate::Error::NoSavePoint(id.to_string()))?;
    Ok(obj.peel_to_commit()?)
}

pub fn describe(project: &Project, commit: &Commit<'_>) -> Result<SavePoint> {
    let meta = notes::read(project, commit.id());
    let raw_message = commit.message().unwrap_or("").to_string();
    let kind = SaveKind::from_message(&raw_message);
    let fallback_title = raw_message
        .lines()
        .next()
        .unwrap_or("세이브 포인트")
        .to_string();

    let at = commit.time().seconds();
    let local: DateTime<Local> = Local
        .timestamp_opt(at, 0)
        .single()
        .unwrap_or_else(Local::now);

    let health = match meta.health.as_deref() {
        Some("ok") => Health::Ok,
        Some("broken") => Health::Broken,
        _ => Health::Unknown,
    };

    // 시작점과 되돌리기는 제목만으로 뜻이 통한다. 요약을 기다리게 하지 않는다.
    let pending_summary =
        meta.summary.is_none() && matches!(kind, SaveKind::Auto | SaveKind::Manual);

    Ok(SavePoint {
        id: short(commit.id()),
        full_id: commit.id().to_string(),
        at,
        at_label: label_time(local),
        title: meta.title.unwrap_or(fallback_title),
        summary: meta.summary,
        kind,
        health,
        checked_by: meta.checked_by,
        broke_because: meta.broke_because,
        files: changed_files(project, commit)?,
        pending_summary,
    })
}

/// 아직 세이브 포인트로 담기지 않은 변경 목록.
pub fn uncommitted(project: &Project) -> Result<Vec<FileChange>> {
    let mut opts = DiffOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let head_tree = project.head_commit().and_then(|c| c.tree().ok());
    let diff = project
        .repo
        .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;
    Ok(collect(&diff))
}

fn changed_files(project: &Project, commit: &Commit<'_>) -> Result<Vec<FileChange>> {
    let tree = commit.tree()?;
    let parent = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let diff = project
        .repo
        .diff_tree_to_tree(parent.as_ref(), Some(&tree), None)?;
    Ok(collect(&diff))
}

fn collect(diff: &git2::Diff<'_>) -> Vec<FileChange> {
    // 줄 수는 파일별로 따로 센 뒤 합친다. delta 목록과 줄 콜백을
    // 같은 벡터에 동시에 쓸 수 없어서 두 단계로 나눈다.
    let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
    let _ = diff.foreach(
        &mut |_delta, _| true,
        None,
        None,
        Some(&mut |delta, _hunk, line| {
            let path = delta_path(&delta);
            let entry = counts.entry(path).or_insert((0, 0));
            match line.origin() {
                '+' => entry.0 += 1,
                '-' => entry.1 += 1,
                _ => {}
            }
            true
        }),
    );

    diff.deltas()
        .map(|delta| {
            let path = delta_path(&delta);
            let (added, removed) = counts.get(&path).copied().unwrap_or((0, 0));
            FileChange {
                kind: match delta.status() {
                    Delta::Added | Delta::Untracked => "새 파일",
                    Delta::Deleted => "삭제",
                    Delta::Renamed => "이름 변경",
                    _ => "수정",
                }
                .to_string(),
                path,
                added,
                removed,
            }
        })
        .collect()
}

fn delta_path(delta: &git2::DiffDelta<'_>) -> String {
    delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

pub fn short(oid: Oid) -> String {
    oid.to_string().chars().take(7).collect()
}

fn label_time(at: DateTime<Local>) -> String {
    let now = Local::now();
    let same_day = at.date_naive() == now.date_naive();
    let hhmm = at.format("%-I:%M").to_string();
    let ampm = if at.format("%p").to_string() == "AM" {
        "오전"
    } else {
        "오후"
    };
    if same_day {
        format!("{ampm} {hhmm}")
    } else {
        format!("{} {ampm} {hhmm}", at.format("%-m월 %-d일"))
    }
}
