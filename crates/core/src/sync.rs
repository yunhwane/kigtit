//! GitHub와 맞추기 — 그리고 "선택이 필요해요".
//!
//! 충돌은 비개발자가 포기하는 지점이다. 대부분 여기서 폴더 복사본을 만든다.
//! Kigtit은 두 가지로 다르게 접근한다.
//!
//! 1. **hunk 단위 병합을 요구하지 않는다.** 코딩을 배운 적 없는 사람에게
//!    `<<<<<<< HEAD` 를 보여 주는 건 아무 도움이 안 된다. 파일 단위로
//!    "내 것" / "저쪽 것"만 고르게 한다. 정직한 단순화다.
//! 2. **반쯤 병합된 상태를 절대 만들지 않는다.** 병합은 메모리에서 계산하고,
//!    충돌이 있으면 작업 폴더를 **건드리지 않은 채** 목록만 돌려준다.
//!    선택이 다 끝났을 때 한 번에 적용한다. 중간에 그만둬도 잃는 게 없다.

use std::time::Duration;

use anyhow::anyhow;
use git2::{Commit, Index, IndexEntry, Oid};
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::repo::Project;
use crate::save::{self, SaveKind};
use crate::timeline::{self, SavePoint};
use crate::tools;

const FETCH_TIMEOUT: Duration = Duration::from_secs(120);

/// 어느 쪽을 남길지.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// 내 컴퓨터에서 한 것
    Mine,
    /// GitHub에 있던 것
    Theirs,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub path: String,
    /// 내 쪽에서 이 파일이 사라졌는가.
    pub mine_deleted: bool,
    pub theirs_deleted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outcome {
    /// 이미 같다.
    UpToDate,
    /// GitHub 쪽 것만 있어서 그대로 가져왔다.
    Pulled { count: usize },
    /// 양쪽에 있었지만 겹치는 파일이 없어서 알아서 합쳤다.
    Merged { count: usize },
    /// 겹쳤다. 작업 폴더는 그대로다.
    NeedsChoice { conflicts: Vec<Conflict> },
    /// 연결된 GitHub이 없다.
    NoRemote,
}

/// GitHub 쪽 변경을 가져와 맞춘다. 충돌이 있으면 아무것도 바꾸지 않는다.
pub fn sync(project: &Project) -> Result<Outcome> {
    let branch = branch_of(project)?;
    if project.repo.find_remote("origin").is_err() {
        return Ok(Outcome::NoRemote);
    }

    // 내 작업을 먼저 담아 둔다. 맞추다가 잃는 경로를 없앤다.
    save::save(project, Some("맞추기 직전 상태"), SaveKind::Auto)?;
    fetch(project, &branch)?;

    let mine = project
        .head_commit()
        .ok_or_else(|| anyhow!("아직 세이브 포인트가 없어요."))?;
    let Some(theirs) = remote_commit(project, &branch) else {
        // GitHub에 이 갈래가 아직 없다. 올리기만 하면 된다.
        return Ok(Outcome::UpToDate);
    };

    if mine.id() == theirs.id() {
        return Ok(Outcome::UpToDate);
    }

    let base_oid = project.repo.merge_base(mine.id(), theirs.id())?;

    // GitHub 쪽이 내 것을 이미 담고 있으면 그냥 따라가면 된다.
    if base_oid == mine.id() {
        let count = count_between(project, mine.id(), theirs.id())?;
        fast_forward(project, &theirs, &branch)?;
        return Ok(Outcome::Pulled { count });
    }
    // 내 쪽이 앞서 있으면 가져올 것이 없다.
    if base_oid == theirs.id() {
        return Ok(Outcome::UpToDate);
    }

    let base = project.repo.find_commit(base_oid)?;
    let mut index =
        project
            .repo
            .merge_trees(&base.tree()?, &mine.tree()?, &theirs.tree()?, None)?;

    if index.has_conflicts() {
        return Ok(Outcome::NeedsChoice {
            conflicts: list_conflicts(&index)?,
        });
    }

    let count = count_between(project, base_oid, theirs.id())?;
    commit_merge(project, &mut index, &mine, &theirs, "GitHub 쪽 변경 합치기")?;
    Ok(Outcome::Merged { count })
}

/// 선택을 받아 한 번에 적용한다. 여기까지 와야 작업 폴더가 바뀐다.
pub fn resolve(project: &Project, choices: &[(String, Side)]) -> Result<SavePoint> {
    let branch = branch_of(project)?;
    let mine = project
        .head_commit()
        .ok_or_else(|| anyhow!("아직 세이브 포인트가 없어요."))?;
    let theirs = remote_commit(project, &branch)
        .ok_or_else(|| anyhow!("GitHub 쪽 내용을 찾을 수 없어요. 다시 맞춰 주세요."))?;

    let base_oid = project.repo.merge_base(mine.id(), theirs.id())?;
    let base = project.repo.find_commit(base_oid)?;
    let mut index =
        project
            .repo
            .merge_trees(&base.tree()?, &mine.tree()?, &theirs.tree()?, None)?;

    let conflicts = list_conflicts(&index)?;
    for conflict in &conflicts {
        let side = choices
            .iter()
            .find(|(p, _)| *p == conflict.path)
            .map(|(_, s)| *s)
            .ok_or_else(|| anyhow!("{}를 어느 쪽으로 둘지 아직 안 골랐어요.", conflict.path))?;
        keep(&mut index, &conflict.path, side, &mine, &theirs)?;
    }

    if index.has_conflicts() {
        return Err(anyhow!("아직 정리되지 않은 파일이 남아 있어요."));
    }

    commit_merge(project, &mut index, &mine, &theirs, "선택한 대로 합치기")
}

/// 겹친 파일에서 양쪽이 무엇을 하려 했는지.
#[derive(Debug, Clone, Serialize)]
pub struct Explanation {
    pub path: String,
    /// 내 컴퓨터에서 이 파일에 무엇을 했는지.
    pub mine: String,
    /// GitHub 쪽에서 무엇을 했는지.
    pub theirs: String,
}

/// 양쪽 변경을 사람 말로 설명한다.
///
/// 코드를 못 읽는 사람에게 `<<<<<<< HEAD`를 보여 주는 건 아무 도움이 안 된다.
/// 무엇을 하려던 변경인지 알아야 고를 수 있다.
pub fn explain(project: &Project, path: &str, agent: crate::ai::Agent) -> Result<Explanation> {
    let branch = branch_of(project)?;
    let mine = project
        .head_commit()
        .ok_or_else(|| anyhow!("아직 세이브 포인트가 없어요."))?;
    let theirs = remote_commit(project, &branch)
        .ok_or_else(|| anyhow!("GitHub 쪽 내용을 찾을 수 없어요."))?;
    let base = project
        .repo
        .find_commit(project.repo.merge_base(mine.id(), theirs.id())?)?;

    Ok(Explanation {
        path: path.to_string(),
        mine: describe_side(project, &base, &mine, path, agent),
        theirs: describe_side(project, &base, &theirs, path, agent),
    })
}

/// 갈라진 지점부터 한쪽까지, 이 파일에 일어난 일.
fn describe_side(
    project: &Project,
    base: &Commit<'_>,
    side: &Commit<'_>,
    path: &str,
    agent: crate::ai::Agent,
) -> String {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);

    let base_tree = base.tree().ok();
    let side_tree = side.tree().ok();
    let Ok(diff) = project.repo.diff_tree_to_tree(
        base_tree.as_ref(),
        side_tree.as_ref(),
        Some(&mut opts),
    ) else {
        return "무엇이 달라졌는지 읽지 못했어요.".into();
    };

    let mut patch = String::new();
    let _ = diff.print(git2::DiffFormat::Patch, |_, _, line| {
        if matches!(line.origin(), '+' | '-' | ' ') {
            patch.push(line.origin());
        }
        patch.push_str(&String::from_utf8_lossy(line.content()));
        true
    });

    if patch.trim().is_empty() {
        return "이 파일은 그대로 뒀어요.".into();
    }
    crate::ai::describe_change(&patch, agent)
}

/// 한 파일에서 고른 쪽 내용만 남긴다.
fn keep(
    index: &mut Index,
    path: &str,
    side: Side,
    mine: &Commit<'_>,
    theirs: &Commit<'_>,
) -> Result<()> {
    let from = match side {
        Side::Mine => mine,
        Side::Theirs => theirs,
    };

    // 충돌 항목(stage 1/2/3)을 모두 걷어낸다.
    index.remove_path(std::path::Path::new(path))?;

    // 고른 쪽에 파일이 없다면 그 쪽에서 지운 것이다. 지운 채로 둔다.
    if let Ok(entry) = from.tree()?.get_path(std::path::Path::new(path)) {
        index.add(&IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: entry.filemode() as u32,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: entry.id(),
            // stage 비트가 0이어야 충돌이 아닌 보통 항목이 된다.
            flags: 0,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        })?;
    }
    Ok(())
}

fn list_conflicts(index: &Index) -> Result<Vec<Conflict>> {
    let mut out = Vec::new();
    for entry in index.conflicts()? {
        let entry = entry?;
        let path = entry
            .our
            .as_ref()
            .or(entry.their.as_ref())
            .or(entry.ancestor.as_ref())
            .map(|e| String::from_utf8_lossy(&e.path).to_string())
            .unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        out.push(Conflict {
            path,
            mine_deleted: entry.our.is_none(),
            theirs_deleted: entry.their.is_none(),
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn commit_merge(
    project: &Project,
    index: &mut Index,
    mine: &Commit<'_>,
    theirs: &Commit<'_>,
    headline: &str,
) -> Result<SavePoint> {
    // 메모리 index이므로 저장소를 지정해 tree를 쓴다.
    let tree_id = index.write_tree_to(&project.repo)?;
    let tree = project.repo.find_tree(tree_id)?;
    let sig = project.signature()?;

    let message = format!("{headline}\n\nKigtit-Kind: {}\n", SaveKind::Manual.as_str());
    let oid = project.repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &message,
        &tree,
        &[mine, theirs],
    )?;

    let created = project.repo.find_commit(oid)?;
    save::checkout_tree(project, &created.tree()?)?;
    timeline::describe(project, &created)
}

fn fast_forward(project: &Project, theirs: &Commit<'_>, branch: &str) -> Result<()> {
    project.repo.reference(
        &format!("refs/heads/{branch}"),
        theirs.id(),
        true,
        "kigtit: GitHub 쪽으로 따라가기",
    )?;
    save::checkout_tree(project, &theirs.tree()?)?;
    Ok(())
}

fn fetch(project: &Project, branch: &str) -> Result<()> {
    // 인증은 사용자가 이미 설정해 둔 수단을 그대로 타야 한다.
    let out = tools::run(
        "git",
        &["fetch", "origin", branch],
        &project.root,
        FETCH_TIMEOUT,
    )
    .map_err(|e| anyhow!(e))?;

    if !out.status.success() {
        let text = tools::text(&out);
        // GitHub에 아직 그 갈래가 없는 경우는 오류가 아니다.
        if text.contains("couldn't find remote ref") {
            return Ok(());
        }
        return Err(anyhow!("GitHub에서 가져오지 못했어요.\n{text}"));
    }
    Ok(())
}

fn remote_commit<'a>(project: &'a Project, branch: &str) -> Option<Commit<'a>> {
    project
        .repo
        .find_reference(&format!("refs/remotes/origin/{branch}"))
        .ok()?
        .peel_to_commit()
        .ok()
}

fn branch_of(project: &Project) -> Result<String> {
    project
        .repo
        .head()?
        .shorthand()
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("어느 갈래인지 알 수 없어요."))
}

/// `from` 이후 `to`까지 몇 개의 세이브 포인트가 있는지.
fn count_between(project: &Project, from: Oid, to: Oid) -> Result<usize> {
    let mut walk = project.repo.revwalk()?;
    walk.push(to)?;
    walk.hide(from)?;
    Ok(walk.count())
}
