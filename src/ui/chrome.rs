//! The editor frame around the buffer: explorer, tab line, status line and
//! command line.

use std::time::{Duration, Instant};

use code_racer_engine::{CodeLanguage, Stats};
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
    ui::{format, theme::Palette},
};

const MEANINGFUL_SPEED_AFTER: Duration = Duration::from_secs(1);
/// Columns below which the context of the status line is left out rather
/// than cut to a stub, its padding included.
const MIN_CONTEXT_WIDTH: usize = 10;
const CONTEXT_SEPARATOR: &str = " · ";

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

/// The entries of the explorer, the ones that key navigation walks, the
/// running session in a folder of its own.
fn explorer_lines(app: &App, width: u16, palette: &Palette) -> Vec<Line<'static>> {
    let mut lines = vec![
        section_title("EXPLORER", palette),
        folder("code-racer", palette),
    ];
    for buffer in app.entries() {
        if buffer == Buffer::Session {
            lines.push(Line::raw(""));
            lines.push(folder("session", palette));
        }
        lines.push(explorer_entry(app, buffer, width, palette));
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
    let label = format::column(&format!("   {marker} {name}"), usize::from(width));
    let style = match (selected, app.focus) {
        (true, Focus::Explorer) => Style::new()
            .bg(palette.highlight)
            .fg(palette.strong)
            .add_modifier(Modifier::BOLD),
        (true, Focus::Editor) => Style::new().fg(palette.accent).add_modifier(Modifier::BOLD),
        (false, _) => Style::new().fg(file_kind(&name, palette).1),
    };
    Line::from(Span::styled(label, style))
}

fn folder(name: &str, palette: &Palette) -> Line<'static> {
    Line::from(Span::styled(
        format!(" ▾ {name}"),
        palette.fg(palette.strong).add_modifier(Modifier::BOLD),
    ))
}

/// The file type of a buffer named `name`, as the status line shows it,
/// and the colour of the name in the explorer, both from its extension.
fn file_kind(name: &str, palette: &Palette) -> (&'static str, Color) {
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);
    match extension {
        Some("toml") => ("toml", palette.kind),
        Some("md") => ("markdown", palette.accent),
        Some("log") => ("log", palette.string),
        _ => match extension.and_then(CodeLanguage::from_extension) {
            Some(language) => (language.name(), palette.text),
            None => ("text", palette.text),
        },
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
            Style::new().bg(palette.panel).fg(palette.text)
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

/// Draws the status line. The statistics on the right always show in full:
/// the context and then the buffer name on the left make room for them.
pub fn statusline(frame: &mut Frame, area: Rect, app: &App, palette: &Palette, now: Instant) {
    let right = right_segments(app, palette, now);
    let room = usize::from(area.width).saturating_sub(spans_width(&right));
    let mut spans = left_segments(app, palette, room);
    let filler = room.saturating_sub(spans_width(&spans));
    spans.push(Span::styled(
        " ".repeat(filler),
        Style::new().bg(palette.panel),
    ));
    spans.extend(right);
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The mode, whether typing is blocked, the buffer name and its context,
/// within `width` columns.
fn left_segments(app: &App, palette: &Palette, width: usize) -> Vec<Span<'static>> {
    let mut spans = vec![mode_badge(app.editor_mode(), palette)];
    if app.is_typing_blocked() {
        spans.push(Span::styled(
            " fix the mistake ",
            palette.badge(palette.error),
        ));
    }
    let mut room = width.saturating_sub(spans_width(&spans));
    if let Some(name) = segment(&app.buffer_name(app.buffer), room) {
        room -= name.width();
        spans.push(Span::styled(
            name,
            Style::new().bg(palette.highlight).fg(palette.strong),
        ));
    }
    if room >= MIN_CONTEXT_WIDTH {
        let context = format::leading_parts(&context(app), CONTEXT_SEPARATOR, room - 2);
        spans.push(Span::styled(
            format!(" {context} "),
            Style::new().bg(palette.panel).fg(palette.text),
        ));
    }
    spans
}

fn mode_badge(mode: EditorMode, palette: &Palette) -> Span<'static> {
    let (label, color) = match mode {
        EditorMode::Normal => (" NORMAL ", palette.accent),
        EditorMode::Insert => (" INSERT ", palette.insert),
        EditorMode::Command => (" COMMAND ", palette.command),
    };
    Span::styled(label, palette.badge(color))
}

/// ` text ` within `width` columns, `None` when not a character fits.
fn segment(text: &str, width: usize) -> Option<String> {
    (width > 2).then(|| format!(" {} ", format::truncate(text, width - 2)))
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// What the buffer is about, from the most telling part to the least.
fn context(app: &App) -> Vec<String> {
    match &app.activity {
        Some(Activity::Solo(run)) if app.buffer == Buffer::Session => run.plan.label_parts(),
        Some(Activity::Race(client)) if app.buffer == Buffer::Session => {
            match (&client.room, client.plan()) {
                (Some(room), Some(plan)) => [format!("room {}", room.code)]
                    .into_iter()
                    .chain(plan.label_parts())
                    .collect(),
                _ => vec![format!("connecting to {}", client.server)],
            }
        }
        _ => vec![app.config.username.clone()],
    }
}

fn right_segments(app: &App, palette: &Palette, now: Instant) -> Vec<Span<'static>> {
    let panel = Style::new().bg(palette.panel).fg(palette.text);
    let accent = Style::new().bg(palette.highlight).fg(palette.strong);
    match (app.buffer, app.session_view()) {
        (Buffer::Session, Some(view)) => {
            let stats = view.stats(now);
            let clock = view.time_left(now).unwrap_or(stats.elapsed);
            vec![
                Span::styled(
                    speed_label(&stats),
                    panel.fg(palette.strong).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {:.0}% ", stats.accuracy), panel),
                Span::styled(error_label(&stats), panel.fg(error_color(&stats, palette))),
                Span::styled(format!(" {} ", format::clock(clock.as_secs())), panel),
                Span::styled(
                    format!(" {:>3}% ", format::percent_done(stats.progress)),
                    accent,
                ),
            ]
        }
        _ => {
            let (file_type, _) = file_kind(&app.buffer_name(app.buffer), palette);
            vec![Span::styled(format!(" {file_type} "), accent)]
        }
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
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            (*key).to_owned(),
            palette.fg(palette.strong).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {action}"), palette.fg(palette.text)));
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
                ("Esc Esc", "abandon"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Theme;

    #[test]
    fn the_file_type_follows_the_extension_of_the_name_shown() {
        let palette = Palette::of(Theme::Editor);
        let names = [
            "race.toml",
            "help.md",
            "FK72AD.md",
            "history.log",
            "main.rs",
            "component.tsx",
            "query.sql",
            "scratch.txt",
            "Makefile",
        ];
        let types = names.map(|name| file_kind(name, &palette).0);
        assert_eq!(
            types,
            [
                "toml",
                "markdown",
                "markdown",
                "log",
                "rust",
                "typescript",
                "sql",
                "text",
                "text"
            ]
        );
    }
}
