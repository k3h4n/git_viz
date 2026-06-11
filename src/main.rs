mod analyzer;
mod git_parser;
mod models;
mod tui_ui;

use std::path::PathBuf;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use analyzer::stats::analyze_repository;
use git_parser::repo::GitRepository;
use tui_ui::app::App;
use tui_ui::event::handle_events;
use tui_ui::render::draw;

#[derive(Parser, Debug)]
#[command(name = "gitviz")]
#[command(about = "A Git repository visualization and analysis tool")]
#[command(version)]
struct Cli {
    #[arg(help = "Path to the git repository")]
    repo_path: Option<PathBuf>,

    #[arg(short, long, default_value = ".")]
    path: PathBuf,

    #[arg(long, help = "Run analysis only and print counts without launching TUI")]
    no_tui: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let repo_path = cli
        .repo_path
        .or(Some(cli.path))
        .unwrap_or_else(|| PathBuf::from("."));

    let repo = GitRepository::open(&repo_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to open repository at '{}': {}",
            repo_path.display(),
            e
        )
    })?;

    eprintln!("Analyzing repository: {} ...", repo_path.display());
    let analysis = analyze_repository(&repo);

    let commits_count = analysis
        .overview
        .as_ref()
        .map(|o| o.total_commits)
        .unwrap_or(0);
    let contributors_count = analysis
        .overview
        .as_ref()
        .map(|o| o.total_contributors)
        .unwrap_or(0);

    eprintln!(
        "Found {} commits, {} contributors",
        commits_count, contributors_count
    );

    if cli.no_tui {
        println!("overview.commits={}", commits_count);
        println!("overview.contributors={}", contributors_count);
        println!(
            "overview.branches={}",
            analysis
                .overview
                .as_ref()
                .map(|o| o.total_branches)
                .unwrap_or(0)
        );
        println!("timeline.entries={}", analysis.timeline.len());
        println!("contributors.entries={}", analysis.contributors.len());
        println!("hotspots.entries={}", analysis.hotspots.len());
        println!(
            "branches.entries={}",
            analysis
                .branch_graph
                .as_ref()
                .map(|b| b.branches.len())
                .unwrap_or(0)
        );
        return Ok(());
    }

    eprintln!("Launching TUI...");

    run_tui(analysis, repo_path.to_string_lossy().to_string())?;

    Ok(())
}

fn run_tui(analysis: models::stats::AnalysisResult, repo_path: String) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(analysis, repo_path);

    while !app.should_quit {
        terminal.draw(|f| draw(f, &app))?;
        handle_events(&mut app)?;
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
