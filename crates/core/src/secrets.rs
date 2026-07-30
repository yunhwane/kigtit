//! 비개발자가 실제로 손해를 보는 세 가지만 막는다: 비밀 키, 대용량 파일, 의존성 폴더.
//! 그 외에는 아무것도 묻지 않는다.

use std::path::Path;

use regex::Regex;
use serde::Serialize;

use crate::Result;
use crate::repo::Project;

/// 5MB 넘는 파일은 백업에 넣지 않는 편이 좋다.
const BIG_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// 이보다 큰 텍스트 파일은 키 검사를 건너뛴다 (빌드 산출물일 가능성이 높다).
const SCAN_LIMIT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// 비밀 키 — 저장을 멈추고 물어본다.
    Secret,
    /// 대용량 파일 — 알려주고 제외를 권한다.
    BigFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub risk: Risk,
    pub path: String,
    /// 비밀 키일 때만 채워진다.
    pub line: Option<usize>,
    /// 사용자에게 보여줄 한 문장.
    pub message: String,
    /// 값 일부만 가린 형태. 전체 값은 절대 밖으로 내보내지 않는다.
    pub masked: Option<String>,
    /// 추천 조치 문구.
    pub advice: String,
}

struct Pattern {
    name: &'static str,
    re: Regex,
}

fn patterns() -> Vec<Pattern> {
    let raw: &[(&str, &str)] = &[
        ("OpenAI key", r"sk-(proj-)?[A-Za-z0-9_-]{20,}"),
        ("Anthropic key", r"sk-ant-[A-Za-z0-9_\-]{20,}"),
        ("AWS access key", r"AKIA[0-9A-Z]{16}"),
        ("Google API key", r"AIza[0-9A-Za-z_\-]{35}"),
        ("GitHub token", r"gh[pousr]_[A-Za-z0-9]{36,}"),
        ("Slack token", r"xox[baprs]-[A-Za-z0-9\-]{10,}"),
        ("Stripe key", r"sk_live_[A-Za-z0-9]{20,}"),
        (
            "private key contents",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
        ),
        (
            "setting that looks like a secret",
            r#"(?i)(api[_-]?key|secret[_-]?key|access[_-]?token|client[_-]?secret|password)\s*[:=]\s*["'][^"'\s]{16,}["']"#,
        ),
    ];
    raw.iter()
        .filter_map(|(name, p)| Regex::new(p).ok().map(|re| Pattern { name, re }))
        .collect()
}

/// 아직 담기지 않은 변경 파일들만 검사한다. 프로젝트 전체를 훑지 않아서 빠르다.
pub fn scan_pending(project: &Project) -> Result<Vec<Finding>> {
    let paths: Vec<String> = crate::timeline::uncommitted(project)?
        .into_iter()
        .filter(|f| f.kind != "Deleted")
        .map(|f| f.path)
        .collect();
    Ok(scan_paths(project, &paths))
}

/// 백업 전에 쓰는 검사. 담기지 않은 변경 + **이미 담긴 모든 파일**을 본다.
///
/// push는 히스토리를 공개한다. 아직 담기지 않은 것만 보면, 이미 커밋된 키가
/// 그대로 새어 나간다.
pub fn scan_tracked(project: &Project) -> Result<Vec<Finding>> {
    let mut paths: Vec<String> = crate::timeline::uncommitted(project)?
        .into_iter()
        .filter(|f| f.kind != "Deleted")
        .map(|f| f.path)
        .collect();

    if let Some(head) = project.head_commit() {
        let tree = head.tree()?;
        tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(git2::ObjectType::Blob) {
                if let Some(name) = entry.name() {
                    paths.push(format!("{dir}{name}"));
                }
            }
            git2::TreeWalkResult::Ok
        })?;
    }

    paths.sort();
    paths.dedup();
    Ok(scan_paths(project, &paths))
}

pub fn scan_paths(project: &Project, rel_paths: &[String]) -> Vec<Finding> {
    let pats = patterns();
    let mut out = Vec::new();

    for rel in rel_paths {
        let abs = project.root.join(rel);
        let Ok(meta) = std::fs::metadata(&abs) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }

        if meta.len() > BIG_FILE_BYTES {
            out.push(Finding {
                risk: Risk::BigFile,
                path: rel.clone(),
                line: None,
                message: format!(
                    "{rel} is {} MB. Including it in backups may slow things down later.",
                    meta.len() / (1024 * 1024)
                ),
                masked: None,
                advice: "Exclude from backups".into(),
            });
            continue;
        }

        if meta.len() > SCAN_LIMIT_BYTES || is_binary(&abs) {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&abs) else {
            continue;
        };

        for (idx, line) in body.lines().enumerate() {
            // 예시 파일에 든 자리표시자는 경고하지 않는다.
            if line.contains("YOUR_") || line.contains("xxxx") || line.contains("여기에") {
                continue;
            }
            if let Some(hit) = pats.iter().find_map(|p| p.re.find(line).map(|m| (p, m))) {
                let (pat, m) = hit;
                out.push(Finding {
                    risk: Risk::Secret,
                    path: rel.clone(),
                    line: Some(idx + 1),
                    message: format!(
                        "Line {} of {rel} contains what looks like a {}. Someone could use it if uploaded.",
                        idx + 1, pat.name
                    ),
                    masked: Some(mask(m.as_str())),
                    advice: "Move the key to an .env file and exclude it from backups".into(),
                });
                break; // 한 파일에서 한 번만 알린다.
            }
        }
    }

    out
}

/// 앞 8자만 남기고 가린다.
fn mask(value: &str) -> String {
    let head: String = value.chars().take(8).collect();
    let tail: String = value
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}••••••{tail}")
}

fn is_binary(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return true;
    };
    let mut buf = [0u8; 1024];
    match f.read(&mut buf) {
        Ok(n) => buf[..n].contains(&0),
        Err(_) => true,
    }
}
