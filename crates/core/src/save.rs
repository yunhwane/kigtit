use git2::{Oid, build::CheckoutBuilder};
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::repo::Project;
use crate::timeline::{self, SavePoint};

/// 세이브 포인트가 왜 만들어졌는지. 커밋 메시지 꼬리표로 남긴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveKind {
    /// 파일이 바뀌고 잠시 조용해져서 자동으로 만든 것
    Auto,
    /// 사용자가 직접 만든 것
    Manual,
    /// 되돌리기가 만든 것 — 되돌림도 기록으로 남는다
    Restore,
    /// 프로젝트를 처음 열었을 때
    Start,
}

const TRAILER: &str = "Kigtit-Kind:";

impl SaveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SaveKind::Auto => "auto",
            SaveKind::Manual => "manual",
            SaveKind::Restore => "restore",
            SaveKind::Start => "start",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SaveKind::Auto => "자동 저장",
            SaveKind::Manual => "직접 저장",
            SaveKind::Restore => "되돌림",
            SaveKind::Start => "시작점",
        }
    }

    pub fn from_message(message: &str) -> Self {
        let value = message
            .lines()
            .rev()
            .find_map(|l| l.trim().strip_prefix(TRAILER))
            .map(str::trim);
        match value {
            Some("manual") => SaveKind::Manual,
            Some("restore") => SaveKind::Restore,
            Some("start") => SaveKind::Start,
            Some("auto") => SaveKind::Auto,
            // Kigtit 밖에서 만든 커밋(예: git commit)은 직접 저장으로 본다.
            _ => SaveKind::Manual,
        }
    }
}

#[derive(Debug)]
// 만들어진 직후 바로 match해서 쓰고 버린다. 컬렉션에 쌓지 않으므로
// 변형 간 크기 차이가 비용이 되지 않는다. Box로 감싸면 호출부만 번거로워진다.
#[allow(clippy::large_enum_variant)]
pub enum SaveOutcome {
    /// 새 세이브 포인트가 만들어졌다.
    Saved(SavePoint),
    /// 바뀐 게 없어서 아무것도 하지 않았다.
    NoChanges,
}

/// 지금 폴더 상태를 세이브 포인트로 담는다.
///
/// 제목을 주지 않으면 파일 개수로 임시 제목을 붙인다. AI 요약이 나중에
/// `refs/notes/kigtit`으로 도착해 화면상 제목을 대체한다.
pub fn save(project: &Project, title: Option<&str>, kind: SaveKind) -> Result<SaveOutcome> {
    stage_all(project)?;

    if !has_staged_changes(project)? {
        return Ok(SaveOutcome::NoChanges);
    }

    let mut index = project.repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = project.repo.find_tree(tree_id)?;
    let sig = project.signature()?;

    let parents: Vec<git2::Commit> = match project.head_commit() {
        Some(c) => vec![c],
        None => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    let kind = if parent_refs.is_empty() {
        SaveKind::Start
    } else {
        kind
    };

    let file_count = timeline::uncommitted(project).map(|f| f.len()).unwrap_or(0);
    let headline = title.map(str::to_owned).unwrap_or_else(|| match kind {
        SaveKind::Start => "프로젝트 시작".to_string(),
        _ if file_count > 0 => format!("{} · 파일 {}개", kind.label(), file_count),
        _ => kind.label().to_string(),
    });
    let message = format!("{headline}\n\n{TRAILER} {}\n", kind.as_str());

    let oid = project
        .repo
        .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parent_refs)?;

    let commit = project.repo.find_commit(oid)?;
    Ok(SaveOutcome::Saved(timeline::describe(project, &commit)?))
}

/// 되돌리기 전에 현재 상태를 반드시 담아둔다. 그래서 잃는 경로가 없다.
pub fn snapshot_before_restore(project: &Project) -> Result<Option<Oid>> {
    match save(project, Some("되돌리기 직전 상태"), SaveKind::Restore)? {
        SaveOutcome::Saved(sp) => Ok(Some(Oid::from_str(&sp.full_id)?)),
        SaveOutcome::NoChanges => Ok(None),
    }
}

fn stage_all(project: &Project) -> Result<()> {
    let mut index = project.repo.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    // 삭제된 파일도 반영한다.
    index.update_all(["*"], None)?;
    index.write()?;
    Ok(())
}

fn has_staged_changes(project: &Project) -> Result<bool> {
    let mut index = project.repo.index()?;
    let tree_id = index.write_tree()?;
    match project.head_commit() {
        Some(head) => Ok(head.tree()?.id() != tree_id),
        // 첫 세이브 포인트 — 빈 폴더가 아니면 담는다.
        None => Ok(!index.is_empty()),
    }
}

/// 작업 폴더를 지정한 tree 내용과 똑같이 맞춘다.
pub(crate) fn checkout_tree(project: &Project, tree: &git2::Tree<'_>) -> Result<()> {
    let mut opts = CheckoutBuilder::new();
    opts.force().remove_untracked(true);
    project
        .repo
        .checkout_tree(tree.as_object(), Some(&mut opts))?;
    Ok(())
}
