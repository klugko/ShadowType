use std::time::{Duration, Instant};

use code_racer_engine::Stats;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use super::{file_kind, spans_width};
use crate::{
    app::{Activity, App, Buffer, EditorMode, SessionView, mouse::Target},
    ui::{format, hits, looks, theme::Palette},
};

const MEANINGFUL_SPEED_AFTER: Duration = Duration::from_secs(1);
/**
 * Columns below which the context of the status line is left out rather
 * than cut to a stub, its padding included.
 */
const MIN_CONTEXT_WIDTH: usize = 10;
const CONTEXT_SEPARATOR: &str = " · ";

/**
 * The statistics on the right always show in full: the context and then the
 * buffer name on the left make room for them.
 */
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
    match (app.buffer, app.session_view()) {
        (Buffer::Session, Some(view)) if app.config.discreet => {
            let (line, column) = cursor_position(&view, palette);
            vec![
                Span::styled(format!(" Ln {line}, Col {column} "), panel),
                Span::styled(" UTF-8 ", panel),
                Span::styled(" LF ", panel),
                file_type(app, palette),
            ]
        }
        (Buffer::Session, Some(view)) => live_stats(&view, palette, now),
        _ => vec![file_type(app, palette)],
    }
}

fn file_type(app: &App, palette: &Palette) -> Span<'static> {
    let (file_type, _) = file_kind(&app.buffer_name(app.buffer), palette);
    Span::styled(format!(" {file_type} "), palette.status_item)
}

fn live_stats(view: &SessionView<'_>, palette: &Palette, now: Instant) -> Vec<Span<'static>> {
    let panel = palette.status;
    let stats = view.stats(now);
    let clock = view.time_left(now).unwrap_or(stats.elapsed);
    vec![
        Span::styled(speed_label(&stats), panel.add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:.0}% ", stats.accuracy), panel),
        Span::styled(error_label(&stats), panel.fg(error_color(&stats, palette))),
        Span::styled(format!(" {} ", format::clock(clock.as_secs())), panel),
        Span::styled(
            format!(" {:>3}% ", format::percent_done(stats.progress)),
            palette.status_item,
        ),
    ]
}

/**
 * The 1-based line and column of the typing cursor in the file the text
 * poses as: the lines a look puts above prose count, and a soft-wrapped line
 * is one line.
 */
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
