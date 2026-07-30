//! GitHub 백업.
//!
//! **토큰을 받지 않는다.** 요약이 로컬 AI CLI를 빌려 쓰는 것과 같은 이유로,
//! 백업은 이미 로그인된 `gh`를 빌려 쓴다. OAuth 앱을 등록할 필요도, 개인
//! 토큰을 만들어 붙여넣을 화면도 없다.
//!
//! 기본값은 **비공개**다. 비개발자가 실수로 세상에 공개하는 쪽이,
//! 나중에 공개로 바꾸는 쪽보다 훨씬 비싸다.

use std::path::Path;
use std::time::Duration;

use anyhow::anyhow;
use serde::Serialize;

use crate::Result;
use crate::repo::Project;
use crate::secrets::{self, Finding, Risk};
use crate::tools;

const GH_TIMEOUT: Duration = Duration::from_secs(60);
const PUSH_TIMEOUT: Duration = Duration::from_secs(180);

/// 백업을 시작할 수 있는 상태인지.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Readiness {
    /// 바로 쓸 수 있다.
    Ready { account: String },
    /// gh는 있지만 로그인이 안 됐다.
    NotSignedIn,
    /// gh가 없다.
    NoTool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub readiness: Readiness,
    /// 이미 연결된 GitHub 주소.
    pub remote: Option<String>,
    /// 아직 백업되지 않은 세이브 포인트 수.
    pub unbacked: usize,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Done {
    pub remote: String,
    pub backed_up: usize,
    /// 이번에 새로 만들었는지.
    pub created: bool,
}

/// 지금 상태. 화면을 그리기 전에 부른다. 네트워크를 타지 않는다.
pub fn status(project: &Project) -> Status {
    let branch = current_branch(project).unwrap_or_else(|| "main".into());
    Status {
        readiness: readiness(&project.root),
        remote: remote_url(project),
        unbacked: unbacked(project, &branch).unwrap_or(0),
        branch,
    }
}

fn readiness(root: &Path) -> Readiness {
    if !tools::exists("gh") {
        return Readiness::NoTool;
    }
    let Ok(out) = tools::run("gh", &["auth", "status"], root, GH_TIMEOUT) else {
        return Readiness::NotSignedIn;
    };
    if !out.status.success() {
        return Readiness::NotSignedIn;
    }
    Readiness::Ready {
        account: account_from(&tools::text(&out)).unwrap_or_else(|| "GitHub".into()),
    }
}

/// `✓ Logged in to github.com account yunhwane (keyring)` 에서 이름만 꺼낸다.
///
/// "account"만 찾으면 아무 문장에서나 엉뚱한 낱말을 계정명으로 집는다.
/// 로그인을 알리는 줄에서만 읽는다.
fn account_from(text: &str) -> Option<String> {
    text.lines()
        .filter(|line| line.contains("Logged in to"))
        .find_map(|line| {
            line.split_whitespace()
                .skip_while(|w| *w != "account")
                .nth(1)
                .map(|s| {
                    s.trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
                        .to_string()
                })
        })
        .filter(|s| !s.is_empty())
}

fn current_branch(project: &Project) -> Option<String> {
    let head = project.repo.head().ok()?;
    head.shorthand().map(str::to_owned)
}

fn remote_url(project: &Project) -> Option<String> {
    let remote = project.repo.find_remote("origin").ok()?;
    remote.url().map(str::to_owned)
}

/// origin이 모르는 세이브 포인트 수. origin이 없으면 전부다.
fn unbacked(project: &Project, branch: &str) -> Result<usize> {
    if !project.has_history() {
        return Ok(0);
    }
    let head = project
        .repo
        .head()?
        .target()
        .ok_or_else(|| anyhow!("아직 세이브 포인트가 없어요."))?;

    let mut walk = project.repo.revwalk()?;
    walk.push(head)?;
    if let Ok(remote_ref) = project
        .repo
        .find_reference(&format!("refs/remotes/origin/{branch}"))
    {
        if let Some(oid) = remote_ref.target() {
            walk.hide(oid)?;
        }
    }
    Ok(walk.count())
}

/// 백업 전에 반드시 통과해야 하는 관문.
///
/// push는 되돌릴 수 없다. 한 번 나간 키는 몇 분 안에 긁혀 간다. 그래서
/// 담기지 않은 변경만이 아니라 **이미 담긴 파일 전부**를 본다.
pub fn guard(project: &Project) -> Result<Vec<Finding>> {
    Ok(secrets::scan_tracked(project)?
        .into_iter()
        .filter(|f| f.risk == Risk::Secret)
        .collect())
}

/// GitHub에 올린다. 연결이 없으면 저장소를 새로 만든다.
///
/// `private`이 기본이어야 한다. 호출하는 쪽에서 명시적으로 공개를 골라야 한다.
pub fn run(project: &Project, private: bool) -> Result<Done> {
    let blocking = guard(project)?;
    if !blocking.is_empty() {
        return Err(anyhow!(
            "비밀 키가 들어 있어서 백업을 멈췄어요. {} — 먼저 정리해 주세요.",
            blocking[0].message
        ));
    }

    if !project.has_history() {
        return Err(anyhow!("아직 세이브 포인트가 없어요. 먼저 저장해 주세요."));
    }
    if matches!(readiness(&project.root), Readiness::NoTool) {
        return Err(anyhow!(
            "GitHub에 올리려면 gh가 필요해요. `brew install gh` 뒤에 `gh auth login`을 한 번 해 주세요."
        ));
    }
    if matches!(readiness(&project.root), Readiness::NotSignedIn) {
        return Err(anyhow!(
            "GitHub 로그인이 필요해요. 터미널에서 `gh auth login`을 한 번 해 주세요."
        ));
    }

    let branch =
        current_branch(project).ok_or_else(|| anyhow!("어느 갈래를 올려야 할지 알 수 없어요."))?;
    let count = unbacked(project, &branch)?;

    let created = match remote_url(project) {
        Some(_) => false,
        None => {
            create_repo(project, private)?;
            true
        }
    };

    push(project, &branch)?;

    Ok(Done {
        remote: remote_url(project).unwrap_or_default(),
        backed_up: count,
        created,
    })
}

fn create_repo(project: &Project, private: bool) -> Result<()> {
    let name = repo_name(&project.root);
    let visibility = if private { "--private" } else { "--public" };

    let out = tools::run(
        "gh",
        &[
            "repo", "create", &name, visibility, "--source", ".", "--remote", "origin",
        ],
        &project.root,
        GH_TIMEOUT,
    )
    .map_err(|e| anyhow!(e))?;

    if !out.status.success() {
        return Err(anyhow!(
            "GitHub에 저장소를 만들지 못했어요.\n{}",
            tools::text(&out)
        ));
    }
    Ok(())
}

/// git2 대신 `git`을 쓴다. 사용자가 이미 설정해 둔 인증 수단을 그대로 타려면
/// 이 편이 확실하다 (gh가 credential helper를 깔아 둔다).
fn push(project: &Project, branch: &str) -> Result<()> {
    let out = tools::run(
        "git",
        &["push", "--set-upstream", "origin", branch],
        &project.root,
        PUSH_TIMEOUT,
    )
    .map_err(|e| anyhow!(e))?;

    if !out.status.success() {
        return Err(anyhow!("올리는 데 실패했어요.\n{}", tools::text(&out)));
    }
    Ok(())
}

/// GitHub 저장소 이름으로 쓸 수 있게 다듬는다. 한글 폴더명도 받아야 한다.
fn repo_name(root: &Path) -> String {
    let raw = root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut out = String::new();
    let mut last_dash = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
            out.push(c);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if trimmed.is_empty() {
        "kigtit-project".into()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_names_survive_korean_folders() {
        assert_eq!(repo_name(Path::new("/tmp/카페 주문 앱")), "kigtit-project");
        assert_eq!(
            repo_name(Path::new("/tmp/cafe order app")),
            "cafe-order-app"
        );
        assert_eq!(repo_name(Path::new("/tmp/my_app.v2")), "my_app.v2");
        assert_eq!(repo_name(Path::new("/tmp/내 blog 사이트")), "blog");
    }

    #[test]
    fn reads_account_from_gh_output() {
        let text = "github.com\n  ✓ Logged in to github.com account yunhwane (keyring)";
        assert_eq!(account_from(text).as_deref(), Some("yunhwane"));
        // 로그인 줄이 아니면 "account"가 들어 있어도 읽지 않는다.
        assert_eq!(account_from("no account here"), None);
        assert_eq!(
            account_from("You are not logged into any GitHub hosts"),
            None
        );
        assert_eq!(account_from(""), None);
    }
}
