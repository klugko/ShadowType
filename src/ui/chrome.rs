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
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{
        Activity, App, Buffer, EditorMode, Focus, MessageKind, SessionView, TextField,
        mouse::Target,
    },
    ui::{
        MIN_HEIGHT, MIN_WIDTH, Moment, SIDEBAR_WIDTH, format, hits, icons, looks, mascot,
        theme::Palette, wrap,
    },
};

const MEANINGFUL_SPEED_AFTER: Duration = Duration::from_secs(1);
/// Most rows a long message takes in the command line.
const MAX_MESSAGE_ROWS: usize = 4;
/// Columns below which the context of the status line is left out rather
/// than cut to a stub, its padding included.
const MIN_CONTEXT_WIDTH: usize = 10;
const CONTEXT_SEPARATOR: &str = " · ";
/// What the command line starts with while a command is typed.
const PROMPT: &str = ":";

pub fn sidebar(frame: &mut Frame, area: Rect, app: &App, palette: &Palette, moment: Moment) {
    let block = Block::new()
        .borders(Borders::RIGHT)
        .border_style(palette.fg(palette.border))
        .style(Style::new().bg(palette.panel).fg(palette.text));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let records = records_section(app, palette);
    let explorer = explorer_lines(app, inner.width, palette);
    let pet = if has_room_for_mascot(app, inner, explorer.len() + records.len()) {
        mascot::HEIGHT
    } else {
        0
    };
    let [files, bottom, home] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(u16::try_from(records.len()).unwrap_or(0)),
        Constraint::Length(pet),
    ])
    .areas(inner);
    for (row, buffer) in entry_rows(app) {
        if row < files.height {
            let entry = Rect::new(files.x, files.y + row, files.width, 1);
            hits::mark(entry, Target::Entry(buffer));
        }
    }
    frame.render_widget(Paragraph::new(explorer), files);
    frame.render_widget(Paragraph::new(records), bottom);
    if pet > 0 {
        let age = if moment.animate {
            app.mascot_age(moment.now)
        } else {
            mascot::STILL
        };
        mascot::render(frame, home, app.mascot_mood(moment.now), age, palette);
    }
}

/// Whether the mascot is on screen: it is wanted, and the explorer has
/// room for it under `lines` lines of its own, a blank line between them.
fn has_room_for_mascot(app: &App, inner: Rect, lines: usize) -> bool {
    app.config.mascot
        && !app.config.discreet
        && inner.width >= mascot::WIDTH
        && usize::from(inner.height) > lines + usize::from(mascot::HEIGHT)
}

/// Whether the mascot shows at the size of the terminal of `app`.
pub fn shows_mascot(app: &App) -> bool {
    let viewport = app.viewport;
    if !app.sidebar || viewport.width < MIN_WIDTH || viewport.height < MIN_HEIGHT {
        return false;
    }
    let body = viewport
        .height
        .saturating_sub(1 + cmdline_height(app, viewport.width));
    let inner = Rect::new(0, 0, SIDEBAR_WIDTH - 1, body);
    let records = records_section(app, &Palette::of(app.config.theme)).len();
    let explorer = explorer_lines(app, inner.width, &Palette::of(app.config.theme)).len();
    has_room_for_mascot(app, inner, explorer + records)
}

/// The entries of the explorer, the ones that key navigation walks, the
/// running session in a folder of its own.
/// The row of each entry of the explorer, as [`explorer_lines`] lays them out.
fn entry_rows(app: &App) -> Vec<(u16, Buffer)> {
    let mut row = 2;
    app.entries()
        .into_iter()
        .map(|buffer| {
            if buffer == Buffer::Session {
                row += 2;
            }
            row += 1;
            (row - 1, buffer)
        })
        .collect()
}

fn explorer_lines(app: &App, width: u16, palette: &Palette) -> Vec<Line<'static>> {
    let mut lines = vec![
        section_title("EXPLORER", palette),
        folder(project_name(app), app, palette),
    ];
    for buffer in app.entries() {
        if buffer == Buffer::Session {
            lines.push(Line::raw(""));
            lines.push(folder("session", app, palette));
        }
        lines.push(explorer_entry(app, buffer, width, palette));
    }
    lines
}

/// An entry of the explorer: its icon, its name, and a dot on the right
/// while it holds a text in progress, as an editor marks unsaved files.
fn explorer_entry(app: &App, buffer: Buffer, width: u16, palette: &Palette) -> Line<'static> {
    let name = app.buffer_name(buffer);
    let selected = app.buffer == buffer;
    let style = match (selected, app.focus) {
        (true, Focus::Explorer) => palette.selection,
        (true, Focus::Editor) => Style::new().fg(palette.accent).add_modifier(Modifier::BOLD),
        (false, _) => Style::new().fg(file_kind(&name, palette).1),
    };
    let marker = if buffer == Buffer::Session && app.session_in_progress() {
        "● "
    } else {
        ""
    };
    let room = usize::from(width).saturating_sub(ENTRY_INDENT + marker.width());
    Line::from(vec![
        Span::styled("   ", style),
        icon(&name, app, style, palette),
        Span::styled(" ", style),
        Span::styled(format::column(&name, room), style),
        Span::styled(marker, style),
    ])
}

/// Columns before the name of an explorer entry: its indentation and icon.
const ENTRY_INDENT: usize = 5;

/// The icon of the file `name` in `style`, coloured by the kind of file;
/// a blank without icons.
fn icon(name: &str, app: &App, style: Style, palette: &Palette) -> Span<'static> {
    match icons::file(name, app.config.icons, palette) {
        Some((glyph, color)) => Span::styled(glyph, style.fg(color)),
        None => Span::styled(" ", style),
    }
}

/// The name the editor goes by: the application's, or in discreet mode
/// the directory it runs in, as for any project.
fn project_name(app: &App) -> &str {
    if app.config.discreet {
        &app.workspace
    } else {
        "code-racer"
    }
}

fn folder(name: &str, app: &App, palette: &Palette) -> Line<'static> {
    let label = match icons::folder(app.config.icons) {
        Some(icon) => format!(" ▾ {icon} {name}"),
        None => format!(" ▾ {name}"),
    };
    Line::from(Span::styled(
        label,
        palette.fg(palette.strong).add_modifier(Modifier::BOLD),
    ))
}

/// The file type of a buffer named `name`, as the status line shows it,
/// and the colour of the name in the explorer, both from its extension.
fn file_kind(name: &str, palette: &Palette) -> (&'static str, Color) {
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);
    match extension {
        _ if name == "COMMIT_EDITMSG" => ("gitcommit", palette.keyword),
        Some("toml") => ("toml", palette.kind),
        Some("md") => ("markdown", palette.accent),
        Some("log") => ("log", palette.string),
        Some("eml") => ("mail", palette.function),
        _ => match extension.and_then(CodeLanguage::from_extension) {
            Some(language) => (language.name(), palette.text),
            None => ("text", palette.text),
        },
    }
}

fn records_section(app: &App, palette: &Palette) -> Vec<Line<'static>> {
    if app.config.discreet {
        return Vec::new();
    }
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
    let today = chrono::Local::now().date_naive();
    let (sessions, time) = app.history.day(today);
    let mut lines = vec![
        section_title("RECORDS", palette),
        stat_line("best", format!("{:.0} wpm", summary.best_wpm), palette),
        stat_line("last 10", format!("{:.0} wpm", summary.recent_wpm), palette),
        stat_line("sessions", summary.sessions.to_string(), palette),
        stat_line(
            "today",
            format!("{sessions} · {} min", time.as_secs().div_ceil(60)),
            palette,
        ),
    ];
    match app.history.streak(today) {
        0 | 1 => {}
        days => lines.push(stat_line("streak", format!("{days} days"), palette)),
    }
    lines
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
        let tab = match icons::file(&name, app.config.icons, palette) {
            Some((glyph, color)) => vec![
                Span::styled(" ", style),
                Span::styled(glyph, style.fg(color)),
                Span::styled(format!(" {name}{modified} "), style),
            ],
            None => vec![Span::styled(format!(" {name}{modified} "), style)],
        };
        let x = area
            .x
            .saturating_add(u16::try_from(spans_width(&spans)).unwrap_or(u16::MAX));
        let width = u16::try_from(spans_width(&tab)).unwrap_or(u16::MAX);
        hits::mark(
            Rect::new(x, area.y, width, 1).intersection(area),
            Target::Tab(buffer),
        );
        spans.extend(tab);
        spans.push(Span::styled(
            "│",
            palette.fg(palette.border).bg(palette.panel),
        ));
    }
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    let brand = format!(" {} ", project_name(app));
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
    if let Some(mode) = spans.first() {
        let width = u16::try_from(mode.width()).unwrap_or(0);
        hits::mark(
            Rect::new(area.x, area.y, width, 1).intersection(area),
            Target::Mode,
        );
    }
    let filler = room.saturating_sub(spans_width(&spans));
    spans.push(Span::styled(" ".repeat(filler), palette.status));
    spans.extend(right);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(palette.status),
        area,
    );
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
        spans.push(Span::styled(name, palette.status_item));
    }
    if room >= MIN_CONTEXT_WIDTH {
        let context = format::leading_parts(&context(app), CONTEXT_SEPARATOR, room - 2);
        spans.push(Span::styled(format!(" {context} "), palette.status));
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
    if app.config.discreet {
        return vec!["⎇ main".to_owned()];
    }
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
    let panel = palette.status;
    let accent = palette.status_item;
    match (app.buffer, app.session_view()) {
        (Buffer::Session, Some(view)) if app.config.discreet => {
            let (file_type, _) = file_kind(&app.buffer_name(app.buffer), palette);
            let (line, column) = cursor_position(&view, palette);
            vec![
                Span::styled(format!(" Ln {line}, Col {column} "), panel),
                Span::styled(" UTF-8 ", panel),
                Span::styled(" LF ", panel),
                Span::styled(format!(" {file_type} "), accent),
            ]
        }
        (Buffer::Session, Some(view)) => {
            let stats = view.stats(now);
            let clock = view.time_left(now).unwrap_or(stats.elapsed);
            vec![
                Span::styled(speed_label(&stats), panel.add_modifier(Modifier::BOLD)),
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

/// The line and column of the typing cursor in the file the text is
/// shown as, both from 1, as an editor counts them: the lines a look puts
/// above prose count, and a soft-wrapped line is one line.
fn cursor_position(view: &SessionView<'_>, palette: &Palette) -> (usize, usize) {
    let session = view.session;
    let typed = &session.target()[..session.cursor()];
    let header = view
        .disguise
        .filter(|_| view.syntax.is_none())
        .map_or(0, |disguise| looks::header(disguise, palette).len());
    let line = header + 1 + typed.iter().filter(|grapheme| *grapheme == "\n").count();
    let column = 1 + typed
        .iter()
        .rev()
        .take_while(|grapheme| *grapheme != "\n")
        .count();
    (line, column)
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
    let calm = palette.status.fg.unwrap_or(palette.text);
    if stats.errors == 0 {
        calm
    } else {
        palette.status_alert
    }
}

/// Draws the command line; returns the cursor position when typing a command.
pub fn cmdline(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    if let Some(prompt) = &app.prompt {
        let width = usize::from(area.width).saturating_sub(PROMPT.width());
        let input = format::input_view(&prompt.input, width);
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("{PROMPT}{}", input.text),
                palette.fg(palette.strong),
            )),
            area,
        );
        let column = PROMPT.width() + input.cursor;
        return Some(Position {
            x: area.x
                + u16::try_from(column)
                    .unwrap_or(u16::MAX)
                    .min(area.width.saturating_sub(1)),
            y: area.y,
        });
    }
    let lines = match app.message() {
        Some(message) => message_rows(&message.text, area.width)
            .into_iter()
            .map(|row| Line::from(Span::styled(row, message_style(message.kind, palette))))
            .collect(),
        None => vec![hints(app, palette)],
    };
    frame.render_widget(Paragraph::new(lines), area);
    None
}

/// Rows the command line takes at `width` columns: those of the message on
/// screen, so that a long one such as a warning with the path of a backup
/// shows whole, and one otherwise.
pub fn cmdline_height(app: &App, width: u16) -> u16 {
    let rows = match (&app.prompt, app.message()) {
        (None, Some(message)) => message_rows(&message.text, width).len(),
        _ => 1,
    };
    u16::try_from(rows.max(1)).unwrap_or(1)
}

/// `text` wrapped at `width` columns, in at most [`MAX_MESSAGE_ROWS`] rows.
fn message_rows(text: &str, width: u16) -> Vec<String> {
    let graphemes: Vec<String> = text.graphemes(true).map(str::to_owned).collect();
    wrap::wrap(&graphemes, width)
        .into_iter()
        .filter(|row| row.start < row.end)
        .take(MAX_MESSAGE_ROWS)
        .map(|row| graphemes[row.start..row.end].concat().trim_end().to_owned())
        .collect()
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
    if app.palette.is_some() {
        return vec![("↑↓", "select"), ("Enter", "run"), ("Esc", "close")];
    }
    if let Some(edit) = &app.editing {
        let action = match edit.field {
            TextField::RoomCode => "join",
            TextField::Username | TextField::Server => "save",
        };
        return vec![("Enter", action), ("Esc", "cancel")];
    }
    if app.is_typing() {
        return match app.activity {
            _ if app.config.discreet => Vec::new(),
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
            ("Ctrl+P", "commands"),
            ("m", "race"),
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
            ("Ctrl+P", "commands"),
        ],
        Buffer::History | Buffer::Help => vec![
            ("j/k", "scroll"),
            ("g/G", "top/bottom"),
            ("Esc", "explorer"),
        ],
        Buffer::Session => match app.activity {
            Some(Activity::Race(_)) => {
                vec![("Esc", "leave"), ("Ctrl+P", "commands"), ("?", "help")]
            }
            _ => vec![
                ("r", "new text"),
                ("Esc", "close"),
                ("Ctrl+P", "commands"),
                ("?", "help"),
            ],
        },
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
