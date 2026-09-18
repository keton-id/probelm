use crate::core::models::Capabilities;
use crate::core::specs::lookup_spec;
use crate::tui::app::{App, InputMode, Tab};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap};
use ratatui::Frame;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Header & Tabs
            Constraint::Min(10),   // Main View
            Constraint::Length(3), // Footer / Status Bar
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);

    match app.active_tab {
        Tab::Models => render_models_tab(f, app, chunks[1]),
        Tab::Adapters => render_adapters_tab(f, app, chunks[1]),
        Tab::Help => render_help_tab(f, chunks[1]),
    }

    render_footer(f, app, chunks[2]);

    if app.input_mode == InputMode::AddingModel {
        render_add_dialog(f, app);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![
        Span::raw(" [1] Models "),
        Span::raw(" [2] Adapters & OAuth "),
        Span::raw(" [3] Help "),
    ];

    let selected_idx = match app.active_tab {
        Tab::Models => 0,
        Tab::Adapters => 1,
        Tab::Help => 2,
    };

    let tabs = Tabs::new(titles)
        .select(selected_idx)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" PROBELM — LLM Gateway & Benchmark Manager ")
                .title_alignment(Alignment::Left),
        )
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn render_models_tab(f: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    let rows: Vec<Row> = app
        .models
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == app.selected_index;
            let check = if item.in_config { " [✓] " } else { " [ ] " };

            let spec = lookup_spec(&item.id);
            let ctx = Capabilities::format_tokens(spec.as_ref().and_then(|s| s.context_window));
            let out = Capabilities::format_tokens(spec.as_ref().and_then(|s| s.max_output));

            let speed = item
                .last_probe
                .as_ref()
                .and_then(|p| p.latency.as_ref())
                .and_then(|l| l.rate_per_sec)
                .map(|r| format!("{:.1} t/s", r))
                .unwrap_or_else(|| "-".to_string());

            let ping = item
                .last_probe
                .as_ref()
                .and_then(|p| p.ping.as_ref())
                .map(|p| if p.ok { "200 OK" } else { "ERR" })
                .unwrap_or("-");

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if item.in_config {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            Row::new(vec![
                Cell::from(check),
                Cell::from(item.id.clone()),
                Cell::from(ctx),
                Cell::from(out),
                Cell::from(ping),
                Cell::from(speed),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Min(30),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(12),
    ];

    let header_cells = ["Cfg", "Model ID", "Context", "Max Output", "Ping", "Speed"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).height(1);

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Models ({} total) ", app.models.len())),
    );

    f.render_widget(table, layout[0]);

    // Bottom pane: Selected model details
    let details = if let Some(item) = app.selected_model() {
        let spec = lookup_spec(&item.id);
        let spec_desc = if let Some(s) = spec {
            format!(
                "Specs: Context={}, MaxOutput={}, Vision={}, Reasoning={}, Tools={}",
                Capabilities::format_tokens(s.context_window),
                Capabilities::format_tokens(s.max_output),
                s.vision.unwrap_or(false),
                s.reasoning.unwrap_or(false),
                s.tools.unwrap_or(false)
            )
        } else {
            "Specs: No cached remote specification found (using gateway defaults)".to_string()
        };

        let probe_desc = if let Some(p) = &item.last_probe {
            let ping_str = p
                .ping
                .as_ref()
                .map(|pi| format!("HTTP Code {}", pi.http_code))
                .unwrap_or_else(|| "Not run".into());
            let lat_str = p
                .latency
                .as_ref()
                .map(|l| {
                    format!(
                        "TTFT: {:.3}s, Total: {:.3}s, Tokens: {}, Speed: {:.1} tok/s",
                        l.ttft_secs.unwrap_or(0.0),
                        l.total_secs.unwrap_or(0.0),
                        l.tokens,
                        l.rate_per_sec.unwrap_or(0.0)
                    )
                })
                .unwrap_or_else(|| "Not run".into());
            format!("Ping: {ping_str} | Latency & Throughput: {lat_str}")
        } else {
            "Probe: Not probed yet in this session. Press 'r' to run benchmark.".to_string()
        };

        format!(
            "Model: {}\nConfigured in config.json: {}\n{}\n{}",
            item.id,
            if item.in_config { "YES" } else { "NO" },
            spec_desc,
            probe_desc
        )
    } else {
        "No model selected.".to_string()
    };

    let details_p = Paragraph::new(details)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Model Details & Benchmark Status "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(details_p, layout[1]);
}

fn render_adapters_tab(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            "Gateway Connector: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.config.base_url),
    ]));
    lines.push(Line::from(vec![
        Span::styled("API Key: ", Style::default().fg(Color::Yellow)),
        Span::raw(if app.config.api_key.len() > 8 {
            format!(
                "{}...{}",
                &app.config.api_key[..4],
                &app.config.api_key[app.config.api_key.len() - 4..]
            )
        } else {
            "***".to_string()
        }),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Detected Local Developer Sessions & OAuth:",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    if app.detected_sessions.is_empty() {
        lines.push(Line::from(
            "  No local Claude Code or GitHub Copilot sessions detected in standard paths.",
        ));
    } else {
        for s in &app.detected_sessions {
            let status_badge = match s.status {
                crate::adap::oauth::SessionStatus::Available => Span::styled(
                    " [ACTIVE] ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                crate::adap::oauth::SessionStatus::Expired => {
                    Span::styled(" [EXPIRED] ", Style::default().fg(Color::Red))
                }
                crate::adap::oauth::SessionStatus::NotConfigured => {
                    Span::styled(" [NOT CONFIGURED] ", Style::default().fg(Color::DarkGray))
                }
            };
            let desc = format!(
                "{} (account: {})",
                s.display_name,
                s.account.as_deref().unwrap_or("unknown")
            );
            lines.push(Line::from(vec![status_badge, Span::raw(desc)]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Roadmap note:",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));
    lines.push(Line::from(Span::styled(
        "  Future adapters can directly bridge detected OAuth tokens to probe non-key based LLMs.",
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Provider Adapters & Connectors "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn render_help_tab(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(Span::styled("Probelm TUI Shortcuts:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  ↑ / k           : Move selection up"),
        Line::from("  ↓ / j           : Move selection down"),
        Line::from("  Space           : Toggle model active/inactive in config.json"),
        Line::from("  a               : Insert / Add new model ID manually"),
        Line::from("  d / Backspace   : Remove selected model from config.json"),
        Line::from("  r / Enter       : Run live benchmark probe (ping + latency + tok/s) on selected model"),
        Line::from("  1 / 2 / 3 / Tab : Switch views between Models, Adapters, and Help"),
        Line::from("  q / Esc         : Quit TUI (changes to config.json are saved automatically)"),
        Line::from(""),
        Line::from(Span::styled("TTY Mode Shortcut:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from("  Use 'probelm config' or 'probelm init' for fast headless or single-prompt updates."),
    ];

    let p = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help & Keyboard Shortcuts "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let status = &app.status_message;
    let help_line = " [q] Quit | [a] Add Model | [Space] Toggle | [r] Probe | [Tab] Switch Tab ";

    let p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow)),
            Span::raw(status),
        ]),
        Line::from(Span::styled(
            help_line,
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(p, area);
}

fn render_add_dialog(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);

    let text = vec![
        Line::from("Enter Model ID to insert into config:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::styled(
                &app.input_buffer,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press <Enter> to confirm, <Esc> to cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Insert Model "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
