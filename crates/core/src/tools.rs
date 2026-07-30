//! 외부 도구 찾기.
//!
//! Finder나 Dock에서 켠 앱은 로그인 셸의 PATH를 물려받지 못한다. launchd가
//! 주는 건 `/usr/bin:/bin:/usr/sbin:/sbin` 정도다. 그래서 `claude`(보통
//! `~/.local/bin`)나 `pnpm`(보통 `/opt/homebrew/bin`)이 없는 것처럼 보이고,
//! 요약과 판정이 조용히 죽는다. 터미널에서 켜면 되던 것이 아이콘을
//! 더블클릭하면 안 되는, 찾기 어려운 종류의 고장이다.
//!
//! 그래서 PATH를 먼저 보고, 없으면 실제로 도구가 깔리는 자리들을 직접 뒤진다.
//! 로그인 셸을 띄워 PATH를 캐내는 방법도 있지만, 프로필이 이상하면 앱 시작이
//! 멈춘다. 정적인 후보 목록이 더 안전하고 빠르다.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// 홈 기준 상대 경로. 순서가 우선순위.
const HOME_DIRS: &[&str] = &[
    ".local/bin",
    ".claude/local",
    ".bun/bin",
    ".cargo/bin",
    ".volta/bin",
    ".deno/bin",
    ".npm-global/bin",
    "go/bin",
];

/// 시스템 전역 후보.
const SYSTEM_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/opt/local/bin",
    "/usr/bin",
    "/bin",
];

/// 실행할 수 있는 도구의 절대 경로. 없으면 None.
pub fn resolve(bin: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(bin);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }

    let home = std::env::var_os("HOME").map(PathBuf::from);
    let from_home = HOME_DIRS
        .iter()
        .filter_map(|rel| home.as_ref().map(|h| h.join(rel)));

    for dir in from_home.chain(SYSTEM_DIRS.iter().map(PathBuf::from)) {
        let candidate = dir.join(bin);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }

    None
}

pub fn exists(bin: &str) -> bool {
    resolve(bin).is_some()
}

/// 외부 명령을 끝까지 기다리되, 시한을 넘기면 끊는다.
///
/// 네트워크를 타는 명령이 멈춰서 앱이 굳는 일을 막는다.
pub fn run(
    bin: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> std::result::Result<Output, String> {
    let program = resolve(bin).ok_or_else(|| format!("Could not find {bin}."))?;

    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| format!("Could not run {bin}."))?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => return Err(format!("A problem occurred while running {bin}.")),
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            return Err(format!("{bin} took too long to respond."));
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    child
        .wait_with_output()
        .map_err(|_| format!("Could not read the result from {bin}."))
}

/// stdout과 stderr를 합친 텍스트. 도구들이 오류를 어느 쪽에 쓸지 일정하지 않다.
pub fn text(out: &Output) -> String {
    let mut s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !err.is_empty() {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(&err);
    }
    s
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_tool_that_must_exist() {
        // PATH가 비어 있어도 시스템 후보에서 찾아야 한다.
        let saved = std::env::var_os("PATH");
        unsafe { std::env::remove_var("PATH") };
        let found = resolve("sh");
        if let Some(p) = saved {
            unsafe { std::env::set_var("PATH", p) };
        }
        assert!(found.is_some(), "PATH 없이도 sh를 찾아야 한다");
    }

    #[test]
    fn missing_tool_is_none() {
        assert!(resolve("kigtit-definitely-not-a-real-binary").is_none());
    }
}
