use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use git2::{Repository, Signature};

use crate::Result;

/// 사용자가 여는 단위. 내부적으로는 git 저장소지만 밖으로는 "프로젝트"다.
pub struct Project {
    pub root: PathBuf,
    pub repo: Repository,
}

/// 비개발자가 실수로 올릴 만한 것들. 프로젝트를 열 때 조용히 넣어준다.
const DEFAULT_IGNORES: &[&str] = &[
    "# Kigtit이 자동으로 넣은 목록입니다. 백업에서 제외됩니다.",
    "node_modules/",
    ".env",
    ".env.*",
    "!.env.example",
    ".DS_Store",
    "dist/",
    "build/",
    ".next/",
    ".nuxt/",
    "out/",
    "target/",
    "__pycache__/",
    ".venv/",
    "venv/",
    "*.log",
    ".pnpm-store/",
    ".turbo/",
    ".cache/",
    "coverage/",
];

impl Project {
    /// 폴더를 연다. git 저장소가 아니면 아무것도 묻지 않고 만든다.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(anyhow!("폴더를 찾을 수 없어요: {}", path.display()));
        }
        let root = path
            .canonicalize()
            .with_context(|| format!("폴더 경로를 읽을 수 없어요: {}", path.display()))?;

        let repo = match Repository::discover(&root) {
            Ok(r) => r,
            Err(_) => {
                // git2의 기본값은 여전히 master다. 사용자 설정과 GitHub 기본값을
                // 따라야 나중에 백업할 때 갈래 이름이 어긋나지 않는다.
                let mut opts = git2::RepositoryInitOptions::new();
                opts.initial_head(&default_branch());
                Repository::init_opts(&root, &opts)
                    .with_context(|| format!("프로젝트를 준비하지 못했어요: {}", root.display()))?
            }
        };

        // workdir()은 끝에 구분자를 붙여 준다. 최근 목록에서 같은 폴더가
        // 두 번 쌓이지 않도록 정규화해 둔다.
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow!("이 폴더는 Kigtit으로 열 수 없어요."))?;
        let root = workdir
            .canonicalize()
            .unwrap_or_else(|_| workdir.to_path_buf());

        let project = Self { root, repo };
        project.ensure_ignores()?;
        Ok(project)
    }

    /// 이미 준비된 프로젝트인지 (세이브 포인트가 하나라도 있는지).
    pub fn has_history(&self) -> bool {
        self.repo.head().ok().and_then(|h| h.target()).is_some()
    }

    pub fn head_commit(&self) -> Option<git2::Commit<'_>> {
        self.repo.head().ok()?.peel_to_commit().ok()
    }

    /// 커밋 작성자. git 설정이 비어 있어도 실패하지 않는다.
    pub fn signature(&self) -> Result<Signature<'static>> {
        if let Ok(sig) = self.repo.signature() {
            // git2가 준 서명은 저장소 수명에 묶이지 않으므로 그대로 복제한다.
            let name = sig.name().unwrap_or("Kigtit").to_string();
            let email = sig.email().unwrap_or("kigtit@localhost").to_string();
            return Ok(Signature::now(&name, &email)?);
        }
        Ok(Signature::now("Kigtit", "kigtit@localhost")?)
    }

    /// .gitignore에 기본 제외 목록을 한 번만 덧붙인다.
    fn ensure_ignores(&self) -> Result<()> {
        let path = self.root.join(".gitignore");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if existing.contains("Kigtit이 자동으로") {
            return Ok(());
        }

        let missing: Vec<&str> = DEFAULT_IGNORES
            .iter()
            .copied()
            .filter(|line| line.starts_with('#') || !ignores(&existing, line))
            .collect();
        if missing.len() <= 1 {
            return Ok(());
        }

        let mut out = existing;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&missing.join("\n"));
        out.push('\n');
        std::fs::write(&path, out).with_context(|| "제외 목록을 저장하지 못했어요.")?;
        Ok(())
    }

    /// 백업에서 빼기 — .gitignore에 한 줄 추가하고 이미 추적 중이면 뺀다.
    pub fn exclude(&self, rel: &str) -> Result<()> {
        let path = self.root.join(".gitignore");
        let mut body = std::fs::read_to_string(&path).unwrap_or_default();
        if !ignores(&body, rel) {
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            body.push_str(rel);
            body.push('\n');
            std::fs::write(&path, body)?;
        }

        let mut index = self.repo.index()?;
        if index.get_path(Path::new(rel), 0).is_some() {
            index.remove_path(Path::new(rel))?;
            index.write()?;
        }
        Ok(())
    }
}

fn ignores(body: &str, line: &str) -> bool {
    body.lines().any(|l| l.trim() == line)
}

/// 새 프로젝트의 첫 갈래 이름. 사용자 설정을 따르고, 없으면 main.
fn default_branch() -> String {
    git2::Config::open_default()
        .ok()
        .and_then(|cfg| cfg.get_string("init.defaultBranch").ok())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "main".to_string())
}
