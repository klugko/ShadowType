//! The editor frame around the buffer: explorer, tab line, status line and
//! command line.

use std::time::{Duration, Instant};

use code_racer_engine::Stats;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{Activity, App, Buffer, EditorMode, Focus, MessageKind, TextField},
    ui::theme::Palette,
};

const MEANINGFUL_SPEED_AFTER: Duration = Duration::from_secs(1);

pub fn sidebar(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) {
    let block = Block::new()
        .borders(Borders::RIGHT)
        .border_style(palette.fg(palette.border))
        .style(Style::new().bg(palette.panel).fg(palette.text));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let records = records_section(app, palette);
    let [files, bottom] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(u16::try_from(records.len()).unwrap_or(0)),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(explorer_lines(app, inner.width, palette)),
        files,
    );
    frame.render_widget(Paragraph::new(records), bottom);
}

fn explorer_lines(app: &App, width: u16, palette: &Palette) -> Vec<Line<'static>> {
    let mut lines = vec![
        section_title("EXPLORER", palette),
        folder("code-racer", palette),
    ];
    for buffer in Buffer::FILES {
        lines.push(explorer_entry(app, buffer, width, palette));
    }
    if app.activity.is_some() {
        lines.push(Line::raw(""));
        lines.push(folder("session", palette));
        lines.push(explorer_entry(app, Buffer::Session, width, palette));
    }
    lines
}

fn explorer_entry(app: &App, buffer: Buffer, width: u16, palette: &Palette) -> Line<'static> {
    let name = app.buffer_name(buffer);
    let selected = app.buffer == buffer;
    let marker = if buffer == Buffer::Session && app.session_in_progress() {
        "●"
    } else {
        " "
    };
    let label = format!("   {marker} {name}");
    let padded = format!("{label:<width$}", width = usize::from(width));
    let style = match (selected, app.focus) {
        (true, Focus::Explorer) => palette
            .cursorline()
            .fg(palette.strong)
            .add_modifier(Modifier::BOLD),
        (true, Focus::Editor) => Style::new().fg(palette.accent).add_modifier(Modifier::BOLD),
        (false, _) => Style::new().fg(file_color(&name, palette)),
    };
    Line::from(Span::styled(padded, style))
}

fn folder(name: &str, palette: &Palette) -> Line<'static> {
    Line::from(Span::styled(
        format!(" ▾ {name}"),
        palette.fg(palette.strong).add_modifier(Modifier::BOLD),
    ))
}

fn file_color(name: &str, palette: &Palette) -> Color {
    match name.rsplit('.').next() {
        Some("toml") => palette.kind,
        Some("md") => palette.accent,
        Some("log") => palette.string,
        _ => palette.text,
    }
}

fn records_section(app: &App, palette: &Palette) -> Vec<Line<'static>> {
    if let Some(Activity::Race(client)) = &app.activity
        && let Some(room) = &client.room
    {
        let mut lines = vec![section_title("ROOM", palette)];
        lines.push(stat_line("code", room.code.to_string(), palette));
        lines.push(stat_line(
            "players",
            format!("{} / {}", room.players.len(), room.max_players),
            palette,
        ));
        return lines;
    }
    let summary = app.history.summary();
    if summary.sessions == 0 {
        return Vec::new();
    }
    vec![
        section_title("RECORDS", palette),
        stat_line("best", format!("{:.0} wpm", summary.best_wpm), palette),
        stat_line("last 10", format!("{:.0} wpm", summary.recent_wpm), palette),
        stat_line("sessions", summary.sessions.to_string(), palette),
    ]
}

fn section_title(title: &str, palette: &Palette) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {title}"),
        palette.fg(palette.muted).add_modifier(Modifier::BOLD),
    ))
}

fn stat_line(label: &str, value: String, palette: &Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("   {label:<9}"), palette.fg(palette.muted)),
        Span::styled(value, palette.fg(palette.text)),
    ])
}

pub fn tabline(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) {
    let mut tabs = vec![app.buffer];
    if app.activity.is_some() && app.buffer != Buffer::Session {
        tabs.push(Buffer::Session);
    }
    let mut spans = Vec::new();
    for buffer in tabs {
        let name = app.buffer_name(buffer);
        let modified = if buffer == Buffer::Session && app.session_in_progress() {
            " ●"
        } else {
            ""
        };
        let style = if buffer == app.buffer {
            Style::new()
                .bg(palette.background)
                .fg(palette.strong)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().bg(palette.panel).fg(palette.muted)
        };
        spans.push(Span::styled(format!(" {name}{modified} "), style));
        spans.push(Span::styled(
            "│",
            palette.fg(palette.border).bg(palette.panel),
        ));
    }
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    let brand = " code-racer ";
    let filler = usize::from(area.width).saturating_sub(used + brand.width());
    spans.push(Span::styled(
        " ".repeat(filler),
        Style::new().bg(palette.panel),
    ));
    spans.push(Span::styled(
        brand,
        Style::new().bg(palette.panel).fg(palette.faint),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub fn statusline(frame: &mut Frame, area: Rect, app: &App, palette: &Palette, now: Instant) {
    let (label, color) = match app.editor_mode() {
        EditorMode::Normal => (" NORMAL ", palette.accent),
        EditorMode::Insert => (" INSERT ", palette.insert),
        EditorMode::Command => (" COMMAND ", palette.command),
    };
    let segment = Style::new().bg(palette.highlight).fg(palette.strong);
    let panel = Style::new().bg(palette.panel).fg(palette.muted);
    let mut left = vec![
        Span::styled(label, palette.badge(color)),
        Span::styled(format!(" {} ", app.buffer_name(app.buffer)), segment),
        Span::styled(format!(" {} ", context(app)), panel),
    ];
    let right = right_segments(app, palette, now);
    let used: usize = left
        .iter()
        .chain(right.iter())
        .map(|span| span.content.width())
        .sum();
    left.push(Span::styled(
        " ".repeat(usize::from(area.width).saturating_sub(used)),
        panel,
    ));
    left.extend(right);
    frame.render_widget(Paragraph::new(Line::from(left)), area);
}

fn context(app: &App) -> String {
    match &app.activity {
        Some(Activity::Solo(run)) if app.buffer == Buffer::Session => run.plan.label(),
        Some(Activity::Race(client)) if app.buffer == Buffer::Session => match &client.room {
            Some(room) => format!("room {} · {}", room.code, client.text_label()),
            None => format!("connecting to {}", client.server),
        },
        _ => app.config.username.clone(),
    }
}

fn right_segments(app: &App, palette: &Palette, now: Instant) -> Vec<Span<'static>> {
    let panel = Style::new().bg(palette.panel).fg(palette.text);
    let accent = Style::new().bg(palette.highlight).fg(palette.strong);
    match (app.buffer, app.session_view()) {
        (Buffer::Session, Some(view)) => {
            let stats = view.session.stats(now);
            let clock = view.session.time_left(now).unwrap_or(stats.elapsed);
            vec![
                Span::styled(
                    speed_label(&stats),
                    panel.fg(palette.strong).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {:.0}% ", stats.accuracy), panel),
                Span::styled(error_label(&stats), panel.fg(error_color(&stats, palette))),
                Span::styled(format!(" {} ", clock_label(clock.as_secs())), panel),
                Span::styled(format!(" {:>3.0}% ", stats.progress * 100.0), accent),
            ]
        }
        _ => vec![Span::styled(format!(" {} ", file_type(app)), accent)],
    }
}

/// Live speed, hidden during the first second when it is mostly noise.
fn speed_label(stats: &Stats) -> String {
    if stats.elapsed < MEANINGFUL_SPEED_AFTER {
        " - wpm ".to_owned()
    } else {
        format!(" {:.0} wpm ", stats.wpm)
    }
}

fn error_label(stats: &Stats) -> String {
    match stats.errors {
        1 => " 1 error ".to_owned(),
        count => format!(" {count} errors "),
    }
}

fn error_color(stats: &Stats, palette: &Palette) -> Color {
    if stats.errors == 0 {
        palette.text
    } else {
        palette.error
    }
}

fn clock_label(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn file_type(app: &App) -> &'static str {
    match app.buffer {
        Buffer::Practice | Buffer::Race | Buffer::Settings => "toml",
        Buffer::History => "log",
        Buffer::Help => "markdown",
        Buffer::Session => "text",
    }
}

/// Draws the command line; returns the cursor position when typing a command.
pub fn cmdline(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    if let Some(prompt) = &app.prompt {
        let text = format!(":{}", prompt.input.value());
        frame.render_widget(
            Paragraph::new(Span::styled(text, palette.fg(palette.strong))),
            area,
        );
        let column = 1 + prompt.input.before_cursor().width();
        return Some(Position {
            x: area.x
                + u16::try_from(column)
                    .unwrap_or(0)
                    .min(area.width.saturating_sub(1)),
            y: area.y,
        });
    }
    let line = match &app.message {
        Some(message) => Line::from(Span::styled(
            message.text.clone(),
            message_style(message.kind, palette),
        )),
        None => hints(app, palette),
    };
    frame.render_widget(Paragraph::new(line), area);
    None
}

fn message_style(kind: MessageKind, palette: &Palette) -> Style {
    match kind {
        MessageKind::Error => palette.fg(palette.error).add_modifier(Modifier::BOLD),
        MessageKind::Info => palette.fg(palette.text),
    }
}

fn hints(app: &App, palette: &Palette) -> Line<'static> {
    let mode = match app.editor_mode() {
        EditorMode::Insert => Span::styled(
            "-- INSERT --  ",
            palette.fg(palette.strong).add_modifier(Modifier::BOLD),
        ),
        _ => Span::raw(""),
    };
    let keys = key_hints(app);
    let mut spans = vec![mode];
    for (index, (key, action)) in keys.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", palette.fg(palette.faint)));
        }
        spans.push(Span::styled(
            (*key).to_owned(),
            palette.fg(palette.muted).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {action}"),
            palette.fg(palette.faint),
        ));
    }
    Line::from(spans)
}

fn key_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    if let Some(edit) = &app.editing {
        let action = match edit.field {
            TextField::RoomCode => "join",
            TextField::Username | TextField::Server => "save",
        };
        return vec![("Enter", action), ("Esc", "cancel")];
    }
    if app.is_typing() {
        return match app.activity {
            Some(Activity::Race(_)) => vec![("Esc Esc", "leave race")],
            _ => vec![
                ("Esc", "stop"),
                ("Ctrl+R", "restart"),
                ("Ctrl+W", "delete word"),
            ],
        };
    }
    if app.focus == Focus::Explorer {
        return vec![
            ("j/k", "move"),
            ("Enter", "open"),
            ("s", "solo"),
            ("m", "race"),
            (":", "command"),
            ("?", "help"),
            ("q", "quit"),
        ];
    }
    match app.buffer {
        Buffer::Practice | Buffer::Race | Buffer::Settings => vec![
            ("j/k", "move"),
            ("h/l", "change"),
            ("Enter", "select"),
            ("Esc", "explorer"),
            (":", "command"),
        ],
        Buffer::History | Buffer::Help => vec![
            ("j/k", "scroll"),
            ("g/G", "top/bottom"),
            ("Esc", "explorer"),
        ],
        Buffer::Session => vec![("Esc", "close"), (":", "command"), ("?", "help")],
    }
}
