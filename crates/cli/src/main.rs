//! kigtit — 터미널에서 쓰는 세이브 포인트.
//!
//! 바이브 코딩은 터미널에서 일어난다. 그래서 앱보다 CLI가 먼저다.
//! 인수 없이 `kigtit`을 치면 지금 폴더의 타임라인을 보여준다.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use kigtit_core::{Agent, Project, Risk, SaveKind, SaveOutcome, ai, restore, secrets, timeline};

#[derive(Parser)]
#[command(
    name = "kigtit",
    about = "Confidence that you can undo — save points for non-developers",
    version,
    disable_help_subcommand = true
)]
struct Cli {
    /// 대상 폴더 (기본값: 지금 폴더)
    #[arg(long, short = 'C', global = true, value_name = "FOLDER")]
    dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// 세이브 포인트를 지금 만든다
    Save {
        /// 제목 (없으면 AI가 붙여준다)
        title: Option<String>,
        /// AI 요약을 기다리지 않는다
        #[arg(long)]
        no_summary: bool,
    },
    /// 타임라인을 보여준다
    List {
        #[arg(long, default_value_t = 12)]
        limit: usize,
    },
    /// 한 세이브 포인트에서 무엇이 바뀌었는지 보여준다
    Show {
        /// 세이브 포인트 id (기본값: 가장 최근)
        id: Option<String>,
        /// 코드도 함께 본다
        #[arg(long)]
        code: bool,
    },
    /// 마지막 세이브 포인트 이전으로 되돌린다
    Undo,
    /// 특정 시점으로 되돌린다
    Back {
        /// 세이브 포인트 id
        id: String,
    },
    /// 비밀 키와 대용량 파일을 검사한다
    Check {
        /// 위험한 파일을 백업에서 바로 빼둔다
        #[arg(long)]
        fix: bool,
    },
    /// 앱이 켜지는지 실제로 돌려 보고 기록한다
    Health,
    /// 앱이 켜지는지 직접 표시한다
    Mark {
        /// ok, broken 또는 unknown
        #[arg(value_parser = ["ok", "broken", "unknown"])]
        state: String,
        id: Option<String>,
    },
    /// 요약이 빠진 세이브 포인트를 채운다
    Summarize {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// GitHub에 백업한다 (기본: 비공개)
    Backup {
        /// 누구나 볼 수 있게 만든다
        #[arg(long)]
        public: bool,
        /// 올리지 않고 지금 상태만 본다
        #[arg(long)]
        status: bool,
    },
    /// GitHub 쪽 변경을 가져와 맞춘다
    Sync {
        /// 겹치는 파일을 한쪽으로 몰아서 정리한다
        #[arg(long, value_parser = ["mine", "theirs"])]
        keep: Option<String>,
    },
    /// 이 폴더를 Kigtit 창으로 연다
    Open,
    /// 폴더를 지켜보며 조용해질 때마다 알아서 저장한다
    Watch {
        /// 파일이 바뀐 뒤 이만큼 조용하면 담는다 (초)
        #[arg(long, default_value_t = 3)]
        idle: u64,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("\n  {err}\n");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let dir = cli.dir.unwrap_or(std::env::current_dir()?);
    let project = Project::open(&dir)?;

    match cli.command {
        None => cmd_list(&project, 12),
        Some(Cmd::List { limit }) => cmd_list(&project, limit),
        Some(Cmd::Save { title, no_summary }) => cmd_save(&project, title, no_summary),
        Some(Cmd::Show { id, code }) => cmd_show(&project, id, code),
        Some(Cmd::Undo) => cmd_restore(&project, None),
        Some(Cmd::Back { id }) => cmd_restore(&project, Some(id)),
        Some(Cmd::Check { fix }) => cmd_check(&project, fix),
        Some(Cmd::Health) => cmd_health(&project),
        Some(Cmd::Mark { state, id }) => cmd_mark(&project, &state, id),
        Some(Cmd::Summarize { limit }) => cmd_summarize(&project, limit),
        Some(Cmd::Backup { public, status }) => cmd_backup(&project, public, status),
        Some(Cmd::Sync { keep }) => cmd_sync(&project, keep),
        Some(Cmd::Open) => cmd_open(&project.root),
        // watch는 자기 Project를 직접 열어 오래 돌아간다.
        Some(Cmd::Watch { idle }) => cmd_watch(&dir, idle),
    }
}

// ── GitHub 백업 ───────────────────────────────────────────

fn cmd_backup(project: &Project, public: bool, status_only: bool) -> Result<()> {
    use kigtit_core::backup::{self, Readiness};

    let status = backup::status(project);
    println!();
    match &status.readiness {
        Readiness::Ready { account } => {
            println!("  \x1b[32m●\x1b[0m  Ready to upload as {account}")
        }
        Readiness::NotSignedIn => {
            println!("  \x1b[33m▲\x1b[0m  GitHub sign-in required");
            println!("     \x1b[2mRun `gh auth login` once in a terminal.\x1b[0m\n");
            return Ok(());
        }
        Readiness::NoTool => {
            println!("  \x1b[33m▲\x1b[0m  gh is required to upload to GitHub");
            println!("     \x1b[2mRun `brew install gh`, then `gh auth login`.\x1b[0m\n");
            return Ok(());
        }
    }

    match &status.remote {
        Some(url) => println!("     \x1b[2m{url}\x1b[0m"),
        None => println!(
            "     \x1b[2mNo repository is connected yet. A new one will be created.\x1b[0m"
        ),
    }
    println!(
        "     \x1b[2m{} save points not backed up\x1b[0m",
        status.unbacked
    );

    if status_only {
        println!();
        return Ok(());
    }
    if status.unbacked == 0 && status.remote.is_some() {
        println!("\n  Everything is already backed up.\n");
        return Ok(());
    }

    // 올리기 전에 관문을 먼저 통과해야 한다. push는 되돌릴 수 없다.
    let blocking = backup::guard(project)?;
    if !blocking.is_empty() {
        println!("\n  \x1b[33m▲ Backup stopped\x1b[0m");
        for f in &blocking {
            println!("     {}", f.message);
            if let Some(m) = &f.masked {
                println!("     \x1b[2m{m}\x1b[0m");
            }
        }
        println!(
            "\n  \x1b[2mExposed keys can be scraped within minutes. Move it to .env first.\x1b[0m\n"
        );
        return Ok(());
    }

    if public {
        println!("\n  \x1b[33m▲\x1b[0m  Uploading \x1b[1mpublicly\x1b[0m.");
    } else {
        println!("\n  \x1b[2mUploading privately.\x1b[0m");
    }
    println!("  \x1b[2mUploading…\x1b[0m");

    let done = backup::run(project, !public)?;
    println!(
        "  \x1b[32m●\x1b[0m  Backed up {} save points\n     \x1b[2m{}\x1b[0m",
        done.backed_up, done.remote
    );
    if done.created {
        println!("     \x1b[2mCreated a new repository.\x1b[0m");
    }
    println!();
    Ok(())
}

// ── 맞추기 ────────────────────────────────────────────────

fn cmd_sync(project: &Project, keep: Option<String>) -> Result<()> {
    use kigtit_core::sync::{self, Outcome, Side};

    println!("\n  \x1b[2mChecking GitHub…\x1b[0m");
    match sync::sync(project)? {
        Outcome::NoRemote => {
            println!("  No GitHub repository is connected yet.");
            println!("  \x1b[2mRun `kigtit backup` first.\x1b[0m\n");
        }
        Outcome::UpToDate => println!("  \x1b[32m●\x1b[0m  Already up to date.\n"),
        Outcome::Pulled { count } => {
            println!("  \x1b[32m●\x1b[0m  Pulled {count} save points from GitHub.\n")
        }
        Outcome::Merged { count } => println!(
            "  \x1b[32m●\x1b[0m  Merged automatically with no overlapping files. Applied {count} from GitHub.\n"
        ),
        Outcome::NeedsChoice { conflicts } => {
            let Some(side) = keep.as_deref() else {
                println!(
                    "\n  \x1b[33m▲ A choice is needed\x1b[0m — these files changed in both places."
                );
                println!("  \x1b[2mYour working folder is unchanged. Nothing was lost.\x1b[0m\n");
                for c in &conflicts {
                    let note = if c.mine_deleted {
                        "  \x1b[2m(deleted on this computer)\x1b[0m"
                    } else if c.theirs_deleted {
                        "  \x1b[2m(deleted on GitHub)\x1b[0m"
                    } else {
                        ""
                    };
                    println!("    \x1b[33m▲\x1b[0m {}{}", c.path, note);
                }
                println!(
                    "\n  \x1b[2mChoose which version to keep:\x1b[0m\n    kigtit sync --keep mine    \x1b[2mkeep changes from this computer\x1b[0m\n    kigtit sync --keep theirs  \x1b[2mkeep changes from GitHub\x1b[0m\n"
                );
                return Ok(());
            };

            let side = if side == "mine" {
                Side::Mine
            } else {
                Side::Theirs
            };
            let choices: Vec<(String, Side)> =
                conflicts.iter().map(|c| (c.path.clone(), side)).collect();
            let sp = sync::resolve(project, &choices)?;
            println!(
                "  \x1b[32m●\x1b[0m  Kept the {} version \x1b[2m{}\x1b[0m  \x1b[1m{}\x1b[0m",
                if side == Side::Mine {
                    "local"
                } else {
                    "GitHub"
                },
                sp.id,
                sp.title
            );
            println!("     \x1b[2mUse `kigtit undo` if you want to go back.\x1b[0m\n");
        }
    }
    Ok(())
}

// ── 창 띄우기 ─────────────────────────────────────────────

/// 설치된 Kigtit 앱을 찾는다. 개발 중에는 빌드 산출물도 본다.
fn find_app() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/Applications/Kigtit.app"),
        dirs_home()?.join("Applications/Kigtit.app"),
    ];
    // target/release/bundle/macos/Kigtit.app — 이 바이너리 옆에서 거슬러 올라간다.
    if let Ok(exe) = std::env::current_exe() {
        for base in exe.ancestors().take(5) {
            candidates.push(base.join("bundle/macos/Kigtit.app"));
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn cmd_open(root: &std::path::Path) -> Result<()> {
    let Some(app) = find_app() else {
        println!(
            "\n  Kigtit is not installed yet.\n  \x1b[2mRun `cargo tauri build`, then put Kigtit.app in Applications.\x1b[0m\n"
        );
        return Ok(());
    };

    std::process::Command::new("open")
        .arg("-a")
        .arg(&app)
        .arg("--args")
        .arg(root)
        .status()?;
    println!(
        "\n  \x1b[35m◆\x1b[0m  Opened Kigtit  \x1b[2m{}\x1b[0m\n",
        root.display()
    );
    Ok(())
}

// ── 자동 저장 ─────────────────────────────────────────────

fn cmd_watch(dir: &std::path::Path, idle_secs: u64) -> Result<()> {
    use kigtit_core::watch::{self, Event};

    let agent = ai::detect();
    println!(
        "\n  \x1b[35m◆\x1b[0m  Autosave on  \x1b[2m{}\x1b[0m",
        dir.display()
    );
    println!(
        "     \x1b[2mSaves after files stay quiet for {idle_secs} seconds. Summaries: {}\x1b[0m",
        agent.label()
    );
    println!("     \x1b[2mPress Ctrl+C to stop\x1b[0m\n");

    watch::watch(
        dir,
        std::time::Duration::from_secs(idle_secs),
        // CLI는 Ctrl+C로 끝나므로 중단 플래그를 쓰지 않는다.
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        |event| match event {
            Event::Changed { files } => {
                println!("  \x1b[2m…\x1b[0m  {files} files changed");
            }
            Event::Saved(sp) => {
                println!(
                    "  \x1b[32m●\x1b[0m  Saved  \x1b[2m{}\x1b[0m  \x1b[1m{}\x1b[0m",
                    sp.id, sp.title
                );
            }
            Event::Resuming { count } => {
                println!(
                    "  \x1b[2m↻\x1b[0m  \x1b[2mFinishing {count} summaries missed while Kigtit was closed\x1b[0m"
                );
            }
            Event::Checked { outcome, .. } => {
                let color = match outcome.health {
                    kigtit_core::Health::Ok => "32",
                    kigtit_core::Health::Broken => "31",
                    kigtit_core::Health::Unknown => "2",
                };
                println!(
                    "  \x1b[{color}m{}\x1b[0m  {}  \x1b[2m({})\x1b[0m",
                    outcome.health.glyph(),
                    outcome.health.label(),
                    outcome.how
                );
                if let Some(detail) = &outcome.detail {
                    for line in detail.lines().take(4) {
                        println!("     \x1b[2m{line}\x1b[0m");
                    }
                }
            }
            Event::Summarized { id, summary } => {
                println!(
                    "  \x1b[2m↳\x1b[0m  \x1b[2m{}\x1b[0m  \x1b[1m{}\x1b[0m — \x1b[2m{}\x1b[0m",
                    &id[..7.min(id.len())],
                    summary.title,
                    summary.summary
                );
            }
            Event::Blocked(findings) => {
                println!("  \x1b[33m▲\x1b[0m  Saving paused — a secret key was found");
                for f in findings.iter().filter(|f| f.risk == Risk::Secret) {
                    println!("     {}", f.message);
                }
                println!("     \x1b[2mRun `kigtit check` to review it\x1b[0m");
            }
        },
    )
}

// ── 타임라인 ──────────────────────────────────────────────

/// 타임라인 레일의 본문 들여쓰기. 한글이 두 칸을 차지하므로 표시 폭으로 계산한다.
const TIME_COL: usize = 11;

fn cmd_list(project: &Project, limit: usize) -> Result<()> {
    let points = timeline::list(project, limit)?;
    let pending = timeline::uncommitted(project)?;

    println!();
    if !pending.is_empty() {
        println!(
            "  \x1b[35m◆\x1b[0m  \x1b[2m{}\x1b[0m  \x1b[1m{} unsaved changes\x1b[0m",
            pad("Now", TIME_COL),
            pending.len()
        );
        let names: Vec<&str> = pending.iter().take(3).map(|f| f.path.as_str()).collect();
        let more = pending.len().saturating_sub(names.len());
        let extra = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        println!("  │{}\x1b[2m{}{}\x1b[0m", rail(), names.join(", "), extra);
        println!("  │{}\x1b[2mUse `kigtit save` to save them\x1b[0m", rail());
        println!("  │");
    }

    if points.is_empty() {
        println!("  No save points yet. Run `kigtit save` to create the first one.\n");
        return Ok(());
    }

    // 앱이 깨진 지점에는 "여기로 가면 된다"를 같이 준다. 그 지점 자체가 아니라
    // 그보다 앞선, 마지막으로 잘 켜졌던 시점을 가리켜야 쓸모가 있다.
    let healthy_before = |idx: usize| -> Option<&kigtit_core::SavePoint> {
        points[idx + 1..]
            .iter()
            .find(|sp| sp.health == kigtit_core::Health::Ok)
    };

    for (i, sp) in points.iter().enumerate() {
        let color = match sp.health {
            kigtit_core::Health::Ok => "32",
            kigtit_core::Health::Broken => "31",
            kigtit_core::Health::Unknown => "2",
        };
        let stem = if i + 1 < points.len() { '│' } else { ' ' };
        println!(
            "  \x1b[{color}m{}\x1b[0m  \x1b[2m{}\x1b[0m  \x1b[1m{}\x1b[0m  \x1b[2m{}\x1b[0m",
            sp.health.glyph(),
            pad(&sp.at_label, TIME_COL),
            sp.title,
            sp.id
        );

        let body = if sp.pending_summary {
            Some("Summarizing…".to_string())
        } else {
            sp.summary.clone()
        };
        if let Some(body) = body {
            println!("  {stem}{}\x1b[2m{}\x1b[0m", rail(), wrap(&body, 62, stem));
        }
        if sp.health == kigtit_core::Health::Broken {
            match healthy_before(i) {
                Some(safe) => println!(
                    "  {stem}{}\x1b[31m{}\x1b[0m  \x1b[2m→ kigtit back {}  ({})\x1b[0m",
                    rail(),
                    sp.health.label(),
                    safe.id,
                    safe.title
                ),
                None => println!(
                    "  {stem}{}\x1b[31m{}\x1b[0m  \x1b[2m→ kigtit undo\x1b[0m",
                    rail(),
                    sp.health.label()
                ),
            }
            // 왜 안 켜지는지 첫 줄만. 전체는 `kigtit show`에서 본다.
            if let Some(why) = sp.broke_because.as_ref().and_then(|d| d.lines().next()) {
                println!("  {stem}{}\x1b[2m{}\x1b[0m", rail(), why.trim());
            }
        }
        if i + 1 < points.len() {
            println!("  │");
        }
    }
    println!();
    Ok(())
}

/// 세이브 포인트 제목이 시작하는 열까지의 공백. 본문 줄을 여기에 맞춘다.
fn rail() -> String {
    " ".repeat(TIME_COL + 4)
}

/// 한글·이모지는 터미널에서 두 칸을 차지한다. 정렬을 이 폭 기준으로 맞춘다.
fn width_of(text: &str) -> usize {
    text.chars()
        .map(|c| if (c as u32) > 0x1100 { 2 } else { 1 })
        .sum()
}

fn pad(text: &str, to: usize) -> String {
    let w = width_of(text);
    format!("{text}{}", " ".repeat(to.saturating_sub(w)))
}

/// 터미널 폭에 맞춰 접는다. 이어지는 줄은 타임라인 레일과 들여쓰기를 맞춘다.
fn wrap(text: &str, width: usize, stem: char) -> String {
    let indent = format!("\n  {stem}{}", rail());
    let mut out = String::new();
    let mut col = 0;
    for word in text.split(' ') {
        let w = width_of(word);
        if col > 0 && col + w > width {
            out.push_str(&indent);
            col = 0;
        }
        if col > 0 {
            out.push(' ');
            col += 1;
        }
        out.push_str(word);
        col += w;
    }
    out
}

// ── 저장 ──────────────────────────────────────────────────

fn cmd_save(project: &Project, title: Option<String>, no_summary: bool) -> Result<()> {
    let findings = secrets::scan_pending(project)?;
    let blocking: Vec<_> = findings.iter().filter(|f| f.risk == Risk::Secret).collect();
    if !blocking.is_empty() {
        println!("\n  \x1b[33m▲ Saving paused\x1b[0m");
        for f in &blocking {
            println!("    {}", f.message);
            if let Some(m) = &f.masked {
                println!("    \x1b[2m{m}\x1b[0m");
            }
        }
        println!("\n  \x1b[2mRun `kigtit check --fix`, then save again.\x1b[0m\n");
        return Ok(());
    }

    let kind = if title.is_some() {
        SaveKind::Manual
    } else {
        SaveKind::Auto
    };
    match kigtit_core::save::save(project, title.as_deref(), kind)? {
        SaveOutcome::NoChanges => {
            println!("\n  Nothing changed. There is nothing to save.\n");
        }
        SaveOutcome::Saved(sp) => {
            println!(
                "\n  \x1b[32m●\x1b[0m  Saved \x1b[2m{}\x1b[0m  \x1b[1m{}\x1b[0m",
                sp.id, sp.title
            );
            for f in &findings {
                println!("     \x1b[33m▲\x1b[0m {}", f.message);
            }
            if no_summary || sp.kind == SaveKind::Start {
                println!();
                return Ok(());
            }

            let agent = ai::detect();
            if agent == Agent::Rules {
                println!(
                    "     \x1b[2mNo AI CLI found, so only the file list is available. Install Claude Code for plain-language explanations.\x1b[0m"
                );
            } else {
                println!("     \x1b[2mSummarizing with {}…\x1b[0m", agent.label());
            }
            match ai::summarize_save_point(project, &sp.full_id, agent) {
                Ok(s) => println!(
                    "     \x1b[1m{}\x1b[0m\n     \x1b[2m{}\x1b[0m\n",
                    s.title, s.summary
                ),
                Err(e) => println!("     \x1b[2mSkipped summary: {e}\x1b[0m\n"),
            }
        }
    }
    Ok(())
}

// ── 보기 ──────────────────────────────────────────────────

fn cmd_show(project: &Project, id: Option<String>, code: bool) -> Result<()> {
    let id = id.unwrap_or_else(|| "HEAD".to_string());
    let sp = timeline::find(project, &id)?;

    println!(
        "\n  \x1b[1m{}\x1b[0m  \x1b[2m{} · {}\x1b[0m",
        sp.title, sp.at_label, sp.id
    );
    if let Some(how) = &sp.checked_by {
        println!(
            "  {}  \x1b[2m{} · Checked by: {}\x1b[0m",
            sp.health.glyph(),
            sp.health.label(),
            how
        );
    }
    if let Some(why) = &sp.broke_because {
        println!();
        for line in why.lines() {
            println!("     \x1b[31m{line}\x1b[0m");
        }
    }
    if let Some(s) = &sp.summary {
        println!("\n  {}", wrap_plain(s, 72));
    } else if sp.pending_summary {
        println!("\n  \x1b[2mNo summary yet. Run `kigtit summarize` to create one.\x1b[0m");
    }

    println!("\n  \x1b[2m{} changed files\x1b[0m", sp.files.len());
    for f in &sp.files {
        println!(
            "    {:<44} \x1b[2m{}\x1b[0m  \x1b[32m+{}\x1b[0m \x1b[31m-{}\x1b[0m",
            f.path, f.kind, f.added, f.removed
        );
    }

    if code {
        let commit = timeline::resolve(project, &id)?;
        println!("\n{}", ai::patch_for(project, &commit)?);
    } else {
        println!("\n  \x1b[2mUse --code to show code\x1b[0m\n");
    }
    Ok(())
}

fn wrap_plain(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    for word in text.split(' ') {
        let w = word.chars().count();
        if col > 0 && col + w > width {
            out.push_str("\n  ");
            col = 0;
        }
        if col > 0 {
            out.push(' ');
            col += 1;
        }
        out.push_str(word);
        col += w;
    }
    out
}

// ── 되돌리기 ──────────────────────────────────────────────

fn cmd_restore(project: &Project, id: Option<String>) -> Result<()> {
    let done = match id {
        Some(id) => restore::restore_to(project, &id)?,
        None => restore::undo(project)?,
    };

    println!(
        "\n  \x1b[32m●\x1b[0m  Restored to \x1b[1m{}\x1b[0m \x1b[2m{}\x1b[0m",
        done.target_title, done.target_id
    );
    if let Some(snap) = &done.snapshot_id {
        println!("     \x1b[2mThe state before restoring was saved as {snap}.\x1b[0m");
    }
    println!("     \x1b[2mYou can undo the restore too → kigtit undo\x1b[0m\n");
    Ok(())
}

// ── 검사 ──────────────────────────────────────────────────

fn cmd_check(project: &Project, fix: bool) -> Result<()> {
    let findings = secrets::scan_pending(project)?;
    if findings.is_empty() {
        println!("\n  \x1b[32m●\x1b[0m  No risky files found.\n");
        return Ok(());
    }

    println!();
    for f in &findings {
        let (glyph, color) = match f.risk {
            Risk::Secret => ("▲", "33"),
            Risk::BigFile => ("■", "33"),
        };
        println!("  \x1b[{color}m{glyph}\x1b[0m  {}", f.message);
        if let Some(m) = &f.masked {
            println!("     \x1b[2m{m}\x1b[0m");
        }
        println!("     \x1b[2mRecommended: {}\x1b[0m", f.advice);

        if fix && f.risk == Risk::BigFile {
            project.exclude(&f.path)?;
            println!("     \x1b[32m→ Excluded from backups.\x1b[0m");
        }
    }

    if fix {
        let left = findings.iter().filter(|f| f.risk == Risk::Secret).count();
        if left > 0 {
            println!(
                "\n  \x1b[2m{left} secret keys were not moved automatically. Move the values to .env and load them from your code.\x1b[0m"
            );
        }
    } else {
        println!("\n  \x1b[2mUse --fix to exclude large files now\x1b[0m");
    }
    println!();
    Ok(())
}

// ── 상태 표시 ─────────────────────────────────────────────

fn cmd_health(project: &Project) -> Result<()> {
    use kigtit_core::health;

    match health::detect(&project.root) {
        // 조사(으로/로)는 라벨 끝글자에 따라 갈리므로 아예 쓰지 않는다.
        Ok(probe) => println!("\n  \x1b[2mChecking… ({})\x1b[0m", probe.label),
        Err(why) => println!("\n  \x1b[2m{why}\x1b[0m"),
    }

    let outcome = health::check_and_mark(project, "HEAD")?;
    let color = match outcome.health {
        kigtit_core::Health::Ok => "32",
        kigtit_core::Health::Broken => "31",
        kigtit_core::Health::Unknown => "2",
    };
    println!(
        "  \x1b[{color}m{}\x1b[0m  {}  \x1b[2m({})\x1b[0m",
        outcome.health.glyph(),
        outcome.health.label(),
        outcome.how
    );

    if let Some(detail) = &outcome.detail {
        println!();
        for line in detail.lines() {
            println!("     \x1b[2m{line}\x1b[0m");
        }
        println!(
            "\n  \x1b[2mRun `kigtit list` to find the last point where the app started.\x1b[0m"
        );
    }
    println!();
    Ok(())
}

fn cmd_mark(project: &Project, state: &str, id: Option<String>) -> Result<()> {
    let id = id.unwrap_or_else(|| "HEAD".to_string());
    let commit = timeline::resolve(project, &id)?;
    let oid = commit.id();
    kigtit_core::notes::merge(
        project,
        oid,
        kigtit_core::notes::Meta {
            health: Some(state.to_string()),
            ..Default::default()
        },
    )?;

    let sp = timeline::find(project, &id)?;
    println!(
        "\n  {}  \x1b[1m{}\x1b[0m — {}\n",
        sp.health.glyph(),
        sp.title,
        sp.health.label()
    );
    Ok(())
}

// ── 요약 채우기 ───────────────────────────────────────────

fn cmd_summarize(project: &Project, limit: usize) -> Result<()> {
    let agent = ai::detect();
    if agent == Agent::Rules {
        println!(
            "\n  \x1b[33m▲\x1b[0m  No AI CLI is available. Only the file list will be shown.\n     \x1b[2mInstall Claude Code or Codex for plain-language explanations.\x1b[0m"
        );
    } else {
        println!("\n  \x1b[2mSummarizing with {}…\x1b[0m", agent.label());
    }

    let done = ai::backfill(project, agent, limit)?;
    if done == 0 {
        println!("  No save points are missing summaries.\n");
    } else {
        println!(
            "  \x1b[32m●\x1b[0m  Filled {done} summaries. Run `kigtit list` to review them.\n"
        );
    }
    Ok(())
}
