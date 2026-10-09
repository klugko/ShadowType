use code_racer_protocol::RoomView;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use super::{cmdline_height, file_kind, project_name};
use crate::{
    app::{Activity, App, Buffer, Focus, mouse::Target},
    history::History,
    ui::{
        MIN_HEIGHT, MIN_WIDTH, Moment, SIDEBAR_WIDTH, format, hits, icons, mascot, theme::Palette,
    },
};

/// Columns before the name of an explorer entry: its indentation and icon.
const ENTRY_INDENT: usize = 5;

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

/**
 * Whether the mascot is wanted and fits under the `lines` lines of the
 * explorer, a blank line between them.
 */
fn has_room_for_mascot(app: &App, inner: Rect, lines: usize) -> bool {
    app.config.mascot
        && !app.config.discreet
        && inner.width >= mascot::WIDTH
        && usize::from(inner.height) > lines + usize::from(mascot::HEIGHT)
}

pub fn shows_mascot(app: &App) -> bool {
    let viewport = app.viewport;
    if !app.sidebar || viewport.width < MIN_WIDTH || viewport.height < MIN_HEIGHT {
        return false;
    }
    let body = viewport
        .height
        .saturating_sub(1 + cmdline_height(app, viewport.width));
    let inner = Rect::new(0, 0, SIDEBAR_WIDTH - 1, body);
    let palette = Palette::of(app.config.theme);
    let lines =
        records_section(app, &palette).len() + explorer_lines(app, inner.width, &palette).len();
    has_room_for_mascot(app, inner, lines)
}

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

/**
 * The dot on the right of an entry holding a text in progress mirrors how an
 * editor marks unsaved files.
 */
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

/// Coloured by the kind of file; a blank when icons are off.
fn icon(name: &str, app: &App, style: Style, palette: &Palette) -> Span<'static> {
    match icons::file(name, app.config.icons, palette) {
        Some((glyph, color)) => Span::styled(glyph, style.fg(color)),
        None => Span::styled(" ", style),
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

fn records_section(app: &App, palette: &Palette) -> Vec<Line<'static>> {
    if app.config.discreet {
        return Vec::new();
    }
    if let Some(Activity::Race(client)) = &app.activity
        && let Some(room) = &client.room
    {
        return room_section(room, palette);
    }
    history_section(&app.history, palette)
}

fn room_section(room: &RoomView, palette: &Palette) -> Vec<Line<'static>> {
    vec![
        section_title("ROOM", palette),
        stat_line("code", room.code.to_string(), palette),
        stat_line(
            "players",
            format!("{} / {}", room.players.len(), room.max_players),
            palette,
        ),
    ]
}

fn history_section(history: &History, palette: &Palette) -> Vec<Line<'static>> {
    let summary = history.summary();
    if summary.sessions == 0 {
        return Vec::new();
    }
    let today = chrono::Local::now().date_naive();
    let (sessions, time) = history.day(today);
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
    match history.streak(today) {
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
