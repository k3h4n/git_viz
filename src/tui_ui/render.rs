use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use crate::analyzer::stats::format_duration;
use crate::models::stats::{BranchInfo, RepoOverview};
use crate::tui_ui::app::{App, ViewMode};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_content(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = ViewMode::ALL
        .iter()
        .map(|v| {
            if *v == app.current_view {
                Line::from(Span::styled(
                    v.title(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(v.title())
            }
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" GitViz - {} ", app.repo_path))
                .style(Style::default().fg(Color::White)),
        )
        .select(app.current_view as usize)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Cyan));

    f.render_widget(tabs, area);
}

fn draw_content(f: &mut Frame, app: &App, area: Rect) {
    match app.current_view {
        ViewMode::Overview => draw_overview(f, app, area),
        ViewMode::Timeline => draw_timeline(f, app, area),
        ViewMode::Contributors => draw_contributors(f, app, area),
        ViewMode::Hotspots => draw_hotspots(f, app, area),
        ViewMode::Branches => draw_branches(f, app, area),
    }
}

fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let overview = match &app.analysis.overview {
        Some(o) => o,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(area);

    draw_overview_stats(f, overview, chunks[0]);
    draw_language_chart(f, overview, chunks[1]);
}

fn draw_overview_stats(f: &mut Frame, overview: &RepoOverview, area: Rect) {
    let duration = if overview.first_commit_date <= overview.latest_commit_date {
        format_duration(overview.first_commit_date, overview.latest_commit_date)
    } else {
        "N/A".to_string()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("  Commits:      ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", overview.total_commits),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Contributors: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", overview.total_contributors),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Branches:     ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", overview.total_branches),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Time Span:    ", Style::default().fg(Color::Yellow)),
            Span::styled(duration, Style::default().fg(Color::White).bold()),
        ]),
        Line::from(vec![
            Span::styled("  First Commit: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", overview.first_commit_date.format("%Y-%m-%d")),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Latest:       ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", overview.latest_commit_date.format("%Y-%m-%d")),
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Repository Overview "),
    );
    f.render_widget(paragraph, area);
}

fn draw_language_chart(f: &mut Frame, overview: &RepoOverview, area: Rect) {
    let mut lang_vec: Vec<_> = overview.language_distribution.iter().collect();
    lang_vec.sort_by(|a, b| b.1.cmp(a.1));

    let total: u64 = lang_vec.iter().map(|(_, v)| **v).sum();
    if total == 0 {
        let p = Paragraph::new("No language data available")
            .block(Block::default().borders(Borders::ALL).title(" Languages "))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            lang_vec
                .iter()
                .map(|_| Constraint::Length(1))
                .chain(std::iter::once(Constraint::Min(0)))
                .collect::<Vec<_>>(),
        )
        .split(area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        }));

    let block = Block::default().borders(Borders::ALL).title(" Languages ");
    f.render_widget(block, area);

    let colors = [
        Color::Cyan,
        Color::Magenta,
        Color::Green,
        Color::Yellow,
        Color::Red,
        Color::Blue,
        Color::Rgb(255, 165, 0),
        Color::Rgb(128, 0, 128),
    ];

    for (i, (lang, count)) in lang_vec.iter().enumerate() {
        if i >= chunks.len() {
            break;
        }
        let ratio = (**count as f64) / (total as f64);
        let color = colors[i % colors.len()];
        let label = format!("{} ({:.1}%)", lang, ratio * 100.0);

        let gauge = Gauge::default()
            .gauge_style(color)
            .label(label)
            .ratio(ratio);
        f.render_widget(gauge, chunks[i]);
    }
}

fn draw_timeline(f: &mut Frame, app: &App, area: Rect) {
    let timeline = app.filtered_timeline();
    let scroll = app.timeline_scroll;

    let items: Vec<ListItem> = timeline
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_selected = i == scroll;
            let branches = if entry.branch_names.is_empty() {
                String::new()
            } else {
                format!(" [{}]", entry.branch_names.join(", "))
            };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line1 = Line::from(vec![
                Span::styled(
                    format!(" {} ", entry.short_hash),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{}", entry.date.format("%Y-%m-%d")),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(branches, Style::default().fg(Color::Magenta)),
            ]);
            let line2 = Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    &entry.message,
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
            ]);
            let line3 = Line::from(vec![
                Span::raw("    "),
                Span::styled(&entry.author, Style::default().fg(Color::Green)),
            ]);

            ListItem::new(vec![line1, line2, line3]).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Commit Timeline ({}/{}) ",
            scroll.saturating_add(1),
            timeline.len()
        )))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    state.select(Some(scroll));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_contributors(f: &mut Frame, app: &App, area: Rect) {
    let contributors = &app.analysis.contributors;
    let scroll = app.contributors_scroll;

    let max_commits = contributors.first().map(|c| c.commit_count).unwrap_or(1);

    let items: Vec<ListItem> = contributors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let is_selected = i == scroll;
            let ratio = c.commit_count as f64 / max_commits as f64;
            let bar_len = (ratio * 30.0) as usize;
            let bar: String = "█".repeat(bar_len);

            let line1 = Line::from(vec![
                Span::styled(
                    format!(" {:>3}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    &c.name,
                    if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::styled(
                    format!(" <{}>", c.email),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            let line2 = Line::from(vec![
                Span::raw("      "),
                Span::styled(bar, Style::default().fg(Color::Green)),
                Span::styled(
                    format!(" {} commits", c.commit_count),
                    Style::default().fg(Color::Yellow),
                ),
            ]);

            ListItem::new(vec![line1, line2])
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Contributors ({}) ", contributors.len())),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    state.select(Some(scroll));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_hotspots(f: &mut Frame, app: &App, area: Rect) {
    let hotspots = &app.analysis.hotspots;
    let scroll = app.hotspots_scroll;

    let max_changes = hotspots.first().map(|h| h.change_count).unwrap_or(1);

    let items: Vec<ListItem> = hotspots
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let is_selected = i == scroll;
            let ratio = h.change_count as f64 / max_changes as f64;
            let bar_len = (ratio * 30.0) as usize;
            let bar: String = "▓".repeat(bar_len);

            let heat_color = if ratio > 0.7 {
                Color::Red
            } else if ratio > 0.4 {
                Color::Yellow
            } else {
                Color::Green
            };

            let line1 = Line::from(vec![
                Span::styled(
                    format!(" {:>3}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    h.path.display().to_string(),
                    if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
            ]);
            let line2 = Line::from(vec![
                Span::raw("      "),
                Span::styled(bar, Style::default().fg(heat_color)),
                Span::styled(
                    format!(" {} changes", h.change_count),
                    Style::default().fg(Color::Yellow),
                ),
            ]);

            ListItem::new(vec![line1, line2])
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" File Hotspots ({}) ", hotspots.len())),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    state.select(Some(scroll));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_branches(f: &mut Frame, app: &App, area: Rect) {
    let branch_graph = match &app.analysis.branch_graph {
        Some(g) => g,
        None => return,
    };

    let scroll = app.branches_scroll;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_branch_list(f, &branch_graph.branches, scroll, chunks[0]);
    draw_merge_list(f, &branch_graph.merge_points, chunks[1]);
}

fn draw_branch_list(f: &mut Frame, branches: &[BranchInfo], scroll: usize, area: Rect) {
    let items: Vec<ListItem> = branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let is_selected = i == scroll;
            let head_marker = if b.is_head { " * " } else { "   " };
            let commit_count = b.commit_hashes.len();

            let style = if b.is_head {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line1 = Line::from(vec![
                Span::styled(head_marker, Style::default().fg(Color::Yellow)),
                Span::styled(
                    &b.name,
                    if b.is_head {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
            ]);
            let line2 = Line::from(vec![
                Span::raw("     "),
                Span::styled(
                    format!(
                        "{} commits | tip: {}",
                        commit_count,
                        &b.commit_hashes
                            .first()
                            .map(|h| h[..7.min(h.len())].to_string())
                            .unwrap_or_default()
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            ListItem::new(vec![line1, line2]).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Branches ({}) ", branches.len())),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    state.select(Some(scroll));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_merge_list(f: &mut Frame, merge_points: &[crate::models::stats::MergePoint], area: Rect) {
    let items: Vec<ListItem> = merge_points
        .iter()
        .map(|m| {
            let short_hash = &m.merge_commit_hash[..7.min(m.merge_commit_hash.len())];
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", short_hash),
                    Style::default().fg(Color::Magenta),
                ),
                Span::raw("merged "),
                Span::styled(&m.source_branch, Style::default().fg(Color::Cyan)),
                Span::raw(" into "),
                Span::styled(&m.target_branch, Style::default().fg(Color::Green)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Merge Points ({}) ", merge_points.len())),
    );
    f.render_widget(list, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let (mode_text, mode_style): (String, Style) = match app.input_mode {
        crate::tui_ui::app::InputMode::Normal => (
            " NORMAL ".to_string(),
            Style::default().fg(Color::Green).bold(),
        ),
        crate::tui_ui::app::InputMode::Search => (
            format!(" SEARCH: {} ", app.search_query),
            Style::default().fg(Color::Yellow).bold(),
        ),
    };

    let help_text = if app.input_mode == crate::tui_ui::app::InputMode::Normal {
        "h/l:switch  j/k:scroll  /:search  q:quit  1-5:goto view"
    } else {
        "Enter/Esc:confirm  type to search"
    };

    let line = Line::from(vec![
        Span::styled(mode_text, mode_style),
        Span::styled(
            format!(" {} ", help_text),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Black));
    f.render_widget(paragraph, area);
}
