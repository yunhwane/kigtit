//! 세이브 포인트에 나중에 덧붙이는 정보 저장소.
//!
//! AI 요약은 저장 후 8초쯤 뒤에 도착한다. 커밋 메시지를 고치면 해시가 바뀌므로
//! `refs/notes/kigtit`에 JSON으로 따로 붙인다. 해시는 그대로 유지된다.

use git2::Oid;
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::repo::Project;

const NOTES_REF: &str = "refs/notes/kigtit";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Meta {
    /// AI가 붙인 사람 말 제목. 없으면 커밋 메시지 첫 줄을 쓴다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// AI가 붙인 두세 문장 요약.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// 이 시점에서 앱이 켜졌는지. "ok" | "broken" | "unknown"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<String>,
    /// 무엇으로 확인했는지. "앱 빌드", "타입 검사" 같은 말.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_by: Option<String>,
    /// 안 켜졌을 때 그 이유. 사용자에게 그대로 보여준다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broke_because: Option<String>,
    /// 요약을 만든 도구 이름 (claude / codex / rules).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

pub fn read(project: &Project, oid: Oid) -> Meta {
    project
        .repo
        .find_note(Some(NOTES_REF), oid)
        .ok()
        .and_then(|n| n.message().map(str::to_owned))
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default()
}

/// 기존 내용을 유지하면서 채워진 필드만 덮어쓴다.
pub fn merge(project: &Project, oid: Oid, patch: Meta) -> Result<Meta> {
    let mut meta = read(project, oid);
    if patch.title.is_some() {
        meta.title = patch.title;
    }
    if patch.summary.is_some() {
        meta.summary = patch.summary;
    }
    if patch.health.is_some() {
        meta.health = patch.health;
        // 판정이 바뀌면 이전 판정의 근거와 이유는 더 이상 맞지 않는다.
        meta.checked_by = patch.checked_by;
        meta.broke_because = patch.broke_because;
    }
    if patch.by.is_some() {
        meta.by = patch.by;
    }

    let sig = project.signature()?;
    let body = serde_json::to_string(&meta)?;
    project
        .repo
        .note(&sig, &sig, Some(NOTES_REF), oid, &body, true)?;
    Ok(meta)
}
