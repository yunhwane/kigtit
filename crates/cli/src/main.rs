//! kigtit — 터미널에서 쓰는 세이브 포인트.
//!
//! 바이브 코딩은 터미널에서 일어난다. 그래서 앱보다 CLI가 먼저다.
//! 인수 없이 `kigtit`을 치면 지금 폴더의 타임라인을 보여준다.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use kigtit_core::{
    Agent, Project, Risk, SaveKind, SaveOutcome, ai, restore, secrets, timeline,
};

#[derive(Parser)]
#[command(
    name = "kigtit",
    about = "되돌릴 수 있다는 확신 — 비개발자를 위한 세이브 포인트",
    version,
    disable_help_subcommand = true
)]
struct Cli {
    /// 대상 폴더 (기본값: 지금 폴더)
    #[arg(long, short = 'C', global = true, value_name = "폴더")]
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
        Some(Cmd::Open) => cmd_open(&project.root),
        // watch는 자기 Project를 직접 열어 오래 돌아간다.
        Some(Cmd::Watch { idle }) => cmd_watch(&dir, idle),
    }
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
            "\n  아직 Kigtit 앱이 설치되지 않았어요.\n  \x1b[2m`cargo tauri build`로 만든 뒤 Kigtit.app을 응용 프로그램 폴더에 넣어 주세요.\x1b[0m\n"
        );
        return Ok(());
    };

    std::process::Command::new("open")
        .arg("-a")
        .arg(&app)
        .arg("--args")
        .arg(root)
        .status()?;
    println!("\n  \x1b[35m◆\x1b[0m  Kigtit 창을 열었어요  \x1b[2m{}\x1b[0m\n", root.display());
    Ok(())
}

// ── 자동 저장 ─────────────────────────────────────────────

fn cmd_watch(dir: &std::path::Path, idle_secs: u64) -> Result<()> {
    use kigtit_core::watch::{self, Event};

    let agent = ai::detect();
    println!(
        "\n  \x1b[35m◆\x1b[0m  자동 저장을 켰어요  \x1b[2m{}\x1b[0m",
        dir.display()
    );
    println!("     \x1b[2m파일이 바뀌고 {idle_secs}초 조용해지면 알아서 담습니다. 요약: {}\x1b[0m", agent.label());
    println!("     \x1b[2m끄려면 Ctrl+C\x1b[0m\n");

    watch::watch(
        dir,
        std::time::Duration::from_secs(idle_secs),
        // CLI는 Ctrl+C로 끝나므로 중단 플래그를 쓰지 않는다.
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        |event| match event {
            Event::Changed { files } => {
                println!("  \x1b[2m…\x1b[0m  파일 {files}개가 바뀌었어요");
            }
            Event::Saved(sp) => {
                println!(
                    "  \x1b[32m●\x1b[0m  담았어요  \x1b[2m{}\x1b[0m  \x1b[1m{}\x1b[0m",
                    sp.id, sp.title
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
                println!("  \x1b[33m▲\x1b[0m  저장을 멈췄어요 — 비밀 키가 들어 있습니다");
                for f in findings.iter().filter(|f| f.risk == Risk::Secret) {
                    println!("     {}", f.message);
                }
                println!("     \x1b[2m`kigtit check`로 확인해 주세요\x1b[0m");
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
            "  \x1b[35m◆\x1b[0m  \x1b[2m{}\x1b[0m  \x1b[1m아직 저장되지 않은 변경 {}개\x1b[0m",
            pad("지금", TIME_COL),
            pending.len()
        );
        let names: Vec<&str> = pending.iter().take(3).map(|f| f.path.as_str()).collect();
        let more = pending.len().saturating_sub(names.len());
        let extra = if more > 0 {
            format!(" 외 {more}개")
        } else {
            String::new()
        };
        println!("  │{}\x1b[2m{}{}\x1b[0m", rail(), names.join(", "), extra);
        println!("  │{}\x1b[2m`kigtit save`로 담을 수 있어요\x1b[0m", rail());
        println!("  │");
    }

    if points.is_empty() {
        println!("  아직 세이브 포인트가 없어요. `kigtit save`로 첫 저장을 만들어 보세요.\n");
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
            Some("요약 중…".to_string())
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
        println!("\n  \x1b[33m▲ 저장을 멈췄어요\x1b[0m");
        for f in &blocking {
            println!("    {}", f.message);
            if let Some(m) = &f.masked {
                println!("    \x1b[2m{m}\x1b[0m");
            }
        }
        println!("\n  \x1b[2m`kigtit check --fix`로 안전하게 정리한 뒤 다시 저장해 주세요.\x1b[0m\n");
        return Ok(());
    }

    let kind = if title.is_some() {
        SaveKind::Manual
    } else {
        SaveKind::Auto
    };
    match kigtit_core::save::save(project, title.as_deref(), kind)? {
        SaveOutcome::NoChanges => {
            println!("\n  바뀐 게 없어요. 저장할 것이 없습니다.\n");
        }
        SaveOutcome::Saved(sp) => {
            println!(
                "\n  \x1b[32m●\x1b[0m  저장했어요 \x1b[2m{}\x1b[0m  \x1b[1m{}\x1b[0m",
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
                    "     \x1b[2mAI CLI가 없어 파일 목록으로만 정리합니다. Claude Code를 설치하면 사람 말로 설명해 드려요.\x1b[0m"
                );
            } else {
                println!("     \x1b[2m{}로 요약하는 중…\x1b[0m", agent.label());
            }
            match ai::summarize_save_point(project, &sp.full_id, agent) {
                Ok(s) => println!("     \x1b[1m{}\x1b[0m\n     \x1b[2m{}\x1b[0m\n", s.title, s.summary),
                Err(e) => println!("     \x1b[2m요약은 건너뛰었어요: {e}\x1b[0m\n"),
            }
        }
    }
    Ok(())
}

// ── 보기 ──────────────────────────────────────────────────

fn cmd_show(project: &Project, id: Option<String>, code: bool) -> Result<()> {
    let id = id.unwrap_or_else(|| "HEAD".to_string());
    let sp = timeline::find(project, &id)?;

    println!("\n  \x1b[1m{}\x1b[0m  \x1b[2m{} · {}\x1b[0m", sp.title, sp.at_label, sp.id);
    if let Some(how) = &sp.checked_by {
        println!(
            "  {}  \x1b[2m{} — {}으로 확인\x1b[0m",
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
        println!("\n  \x1b[2m아직 요약이 없어요. `kigtit summarize`로 만들 수 있습니다.\x1b[0m");
    }

    println!("\n  \x1b[2m바뀐 파일 {}개\x1b[0m", sp.files.len());
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
        println!("\n  \x1b[2m코드로 보려면 --code\x1b[0m\n");
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
        "\n  \x1b[32m●\x1b[0m  \x1b[1m{}\x1b[0m 시점으로 되돌렸어요 \x1b[2m{}\x1b[0m",
        done.target_title, done.target_id
    );
    if let Some(snap) = &done.snapshot_id {
        println!("     \x1b[2m되돌리기 직전 상태는 {snap}에 담아뒀어요.\x1b[0m");
    }
    println!(
        "     \x1b[2m되돌린 것도 되돌릴 수 있어요 → kigtit undo\x1b[0m\n"
    );
    Ok(())
}

// ── 검사 ──────────────────────────────────────────────────

fn cmd_check(project: &Project, fix: bool) -> Result<()> {
    let findings = secrets::scan_pending(project)?;
    if findings.is_empty() {
        println!("\n  \x1b[32m●\x1b[0m  위험한 파일이 없어요.\n");
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
        println!("     \x1b[2m추천: {}\x1b[0m", f.advice);

        if fix && f.risk == Risk::BigFile {
            project.exclude(&f.path)?;
            println!("     \x1b[32m→ 백업에서 뺐어요.\x1b[0m");
        }
    }

    if fix {
        let left = findings.iter().filter(|f| f.risk == Risk::Secret).count();
        if left > 0 {
            println!(
                "\n  \x1b[2m비밀 키 {left}건은 자동으로 옮기지 않았어요. 값을 .env로 옮기고 코드에서는 불러오도록 바꿔 주세요.\x1b[0m"
            );
        }
    } else {
        println!("\n  \x1b[2m대용량 파일을 바로 정리하려면 --fix\x1b[0m");
    }
    println!();
    Ok(())
}

// ── 상태 표시 ─────────────────────────────────────────────

fn cmd_health(project: &Project) -> Result<()> {
    use kigtit_core::health;

    match health::detect(&project.root) {
        Ok(probe) => println!("\n  \x1b[2m{}으로 확인하는 중…\x1b[0m", probe.label),
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
        println!("\n  \x1b[2m마지막으로 잘 켜졌던 시점으로 돌아가려면 `kigtit list`를 보세요.\x1b[0m");
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
            "\n  \x1b[33m▲\x1b[0m  쓸 수 있는 AI CLI가 없어요. 파일 목록으로만 정리합니다.\n     \x1b[2mClaude Code나 Codex를 설치하면 사람 말로 설명해 드려요.\x1b[0m"
        );
    } else {
        println!("\n  \x1b[2m{}로 요약하는 중…\x1b[0m", agent.label());
    }

    let done = ai::backfill(project, agent, limit)?;
    if done == 0 {
        println!("  요약이 빠진 세이브 포인트가 없어요.\n");
    } else {
        println!("  \x1b[32m●\x1b[0m  {done}개를 채웠어요. `kigtit list`로 확인해 보세요.\n");
    }
    Ok(())
}
