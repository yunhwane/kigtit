//! Kigtit core — 비개발자용 세이브 포인트 엔진.
//!
//! 이 크레이트가 CLI(`crates/cli`)와 데스크톱 앱(`src-tauri`)의 공통 로직을 담는다.
//! 바깥으로 나가는 모든 문장은 한국어이고 git 용어를 쓰지 않는다.

pub mod ai;
pub mod health;
pub mod notes;
pub mod repo;
pub mod restore;
pub mod save;
pub mod secrets;
pub mod timeline;
pub mod watch;

pub use ai::{Agent, Summary};
pub use health::Outcome;
pub use repo::Project;
pub use restore::Restored;
pub use save::{SaveKind, SaveOutcome};
pub use secrets::{Finding, Risk};
pub use timeline::{FileChange, Health, SavePoint};

/// 사용자에게 그대로 보여줄 수 있는 오류.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("폴더를 찾을 수 없어요: {0}")]
    NoFolder(String),

    #[error("아직 세이브 포인트가 하나도 없어요. 먼저 저장해 주세요.")]
    Empty,

    #[error("'{0}' 세이브 포인트를 찾을 수 없어요.")]
    NoSavePoint(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, anyhow::Error>;
