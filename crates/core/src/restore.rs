//! 되돌리기.
//!
//! 핵심 규칙: **작업이 영구히 사라지는 경로가 존재하지 않는다.**
//! `reset --hard`를 쓰지 않는다. 되돌리기는 (1) 현재 상태를 먼저 담고,
//! (2) 목표 시점의 내용을 담은 **새 세이브 포인트**를 얹는 방식이다.
//! 그래서 되돌린 것도 되돌릴 수 있다.

use anyhow::anyhow;
use serde::Serialize;

use crate::Result;
use crate::repo::Project;
use crate::save::{self, SaveKind};
use crate::timeline::{self, Health, SavePoint};

#[derive(Debug, Serialize)]
pub struct Restored {
    /// 되돌리기 결과로 새로 만들어진 세이브 포인트.
    pub created: SavePoint,
    /// 어느 시점으로 되돌렸는지.
    pub target_id: String,
    pub target_title: String,
    /// 되돌리기 직전 상태를 따로 담았다면 그 id. 여기로 다시 돌아올 수 있다.
    pub snapshot_id: Option<String>,
}

/// 지정한 세이브 포인트 시점의 내용으로 되돌린다.
pub fn restore_to(project: &Project, id: &str) -> Result<Restored> {
    // 저장하기 전에 목표를 먼저 확정한다. 아래에서 HEAD가 움직이기 때문이다.
    let target = timeline::resolve(project, id)?;
    let target_id = timeline::short(target.id());
    let target_title = timeline::describe(project, &target)?.title;
    let target_tree = target.tree()?;

    // 1. 지금 상태를 잃지 않도록 먼저 담는다.
    let snapshot = save::snapshot_before_restore(project)?;

    let head = project
        .head_commit()
        .ok_or_else(|| anyhow!("아직 세이브 포인트가 하나도 없어요."))?;

    if head.tree()?.id() == target_tree.id() {
        return Err(anyhow!("이미 그 시점과 똑같은 상태예요."));
    }

    // 2. 목표 시점의 내용을 그대로 담은 새 세이브 포인트를 얹는다.
    let sig = project.signature()?;
    let message = format!(
        "되돌림: {target_title}\n\nKigtit-Kind: {}\nKigtit-Restored-From: {target_id}\n",
        SaveKind::Restore.as_str()
    );
    let oid = project.repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &message,
        &target_tree,
        &[&head],
    )?;

    // 3. 작업 폴더를 새 세이브 포인트에 맞춘다.
    let created = project.repo.find_commit(oid)?;
    save::checkout_tree(project, &created.tree()?)?;

    Ok(Restored {
        created: timeline::describe(project, &created)?,
        target_id,
        target_title,
        snapshot_id: snapshot.map(timeline::short),
    })
}

/// 마지막 세이브 포인트 이전으로. `kigtit undo`가 쓴다.
pub fn undo(project: &Project) -> Result<Restored> {
    let head = project
        .head_commit()
        .ok_or_else(|| anyhow!("아직 세이브 포인트가 하나도 없어요."))?;
    let parent = head
        .parent(0)
        .map_err(|_| anyhow!("여기가 시작점이라 더 되돌릴 수 없어요."))?;
    restore_to(project, &parent.id().to_string())
}

/// 앱이 마지막으로 잘 켜졌던 시점. 되돌리기 화면의 기본 추천값.
pub fn last_healthy(project: &Project) -> Result<Option<SavePoint>> {
    Ok(timeline::list(project, 200)?
        .into_iter()
        .find(|sp| sp.health == Health::Ok))
}
