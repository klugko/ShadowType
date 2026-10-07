//! A solo session: the text while typing, then the results.

use code_racer_engine::Status;
use ratatui::{Frame, layout::Rect, style::Modifier, text::Span};

use super::doc;
use crate::{
    app::{
        App, SessionView,
        practice::{SoloResult, SoloRun},
    },
    ui::{
        editor::{self, Row},
        format::race_time,
        theme::Palette,
        typing,
    },
};

const RESULT_KEY_WIDTH: usize = 11;
/// Tallest the speed chart of the results gets.
const MAX_CHART_HEIGHT: usize = 8;
/// Fewest rows worth drawing the speed chart in.
const MIN_CHART_HEIGHT: usize = 4;

pub fn render(frame: &mut Frame, area: Rect, app: &App, run: &SoloRun, palette: &Palette) {
    match &run.result {
        Some(result) => results(frame, area, run, result, palette),
        None => text(frame, area, app, palette),
    }
}

fn text(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) {
    let Some(view) = app.session_view() else {
        return;
    };
    let (mut rows, cursor_row) = text_rows(&view, area, palette, app.is_typing());
    if let Some(source) = view.attribution {
        rows.push(Row::blank());
        rows.push(Row::new(vec![Span::styled(
            format!("-- {source}"),
            palette.fg(palette.comment).add_modifier(Modifier::ITALIC),
        )]));
    }
    let scroll = editor::scroll_for(cursor_row, area.height, rows.len());
    editor::render(frame, area, &rows, scroll, palette);
}

/// Rows of a session text laid out for `area`, and the row of the cursor.
pub fn text_rows(
    view: &SessionView<'_>,
    area: Rect,
    palette: &Palette,
    active: bool,
) -> (Vec<Row>, usize) {
    let mut highest = 999;
    loop {
        let width = editor::text_width(area.width, highest);
        let layout = typing::layout(view, width, palette, active);
        if layout.rows.len() <= highest {
            return (layout.rows, layout.cursor_row);
        }
        highest = layout.rows.len();
    }
}

fn results(frame: &mut Frame, area: Rect, run: &SoloRun, result: &SoloResult, palette: &Palette) {
    let stats = result.stats;
    let heading = match run.session.status() {
        Status::TimeUp => "time's up",
        _ => "session complete",
    };
    let mut rows = vec![
        doc::title(heading, palette),
        doc::comment(run.plan.label(), palette),
        doc::blank(),
        metric(
            "wpm",
            format!("{:.1}", stats.wpm),
            palette.fg(palette.number).add_modifier(Modifier::BOLD),
            palette,
        ),
        metric(
            "raw",
            format!("{:.1}", stats.raw_wpm),
            palette.fg(palette.number),
            palette,
        ),
        metric(
            "accuracy",
            format!("{:.1}%", stats.accuracy),
            palette.fg(palette.number),
            palette,
        ),
        metric(
            "errors",
            stats.errors.to_string(),
            palette.fg(palette.number),
            palette,
        ),
        metric(
            "characters",
            format!(
                "{} correct, {} incorrect",
                stats.correct_chars, stats.incorrect_chars
            ),
            palette.fg(palette.text),
            palette,
        ),
        metric(
            "consistency",
            format!("{:.0}%", result.consistency),
            palette.fg(palette.number),
            palette,
        ),
        metric(
            "time",
            race_time(u64::try_from(stats.elapsed.as_millis()).unwrap_or(u64::MAX)),
            palette.fg(palette.number),
            palette,
        ),
        doc::blank(),
        personal_best(result, palette),
    ];
    let footer = [
        doc::blank(),
        doc::keys(
            &[("r", "new text"), ("e", "settings"), ("Esc", "close")],
            palette,
        ),
    ];
    let wpm: Vec<f64> = result.samples.iter().map(|sample| sample.wpm).collect();
    let heading = [doc::blank(), doc::heading("wpm over time", palette)];
    let room = usize::from(area.height).saturating_sub(rows.len() + heading.len() + footer.len());
    let height = room.min(MAX_CHART_HEIGHT);
    if wpm.len() >= 2 && height >= MIN_CHART_HEIGHT {
        let lines = rows.len() + heading.len() + height + footer.len();
        let width = editor::text_width(area.width, lines);
        let height = u16::try_from(height).unwrap_or(u16::MAX);
        rows.extend(heading);
        rows.extend(doc::chart(&wpm, width, height, palette));
    }
    rows.extend(footer);
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

fn metric(name: &str, value: String, style: ratatui::style::Style, palette: &Palette) -> Row {
    Row::new(doc::assignment(
        name,
        RESULT_KEY_WIDTH,
        Span::styled(value, style),
        palette,
    ))
}

fn personal_best(result: &SoloResult, palette: &Palette) -> Row {
    match result.previous_best {
        _ if result.is_personal_best() => Row::new(vec![Span::styled(
            match result.previous_best {
                Some(best) => format!("new personal best, previous {best:.1} wpm"),
                None => "first result for this mode, it is your personal best".to_owned(),
            },
            palette.fg(palette.success).add_modifier(Modifier::BOLD),
        )]),
        Some(best) => doc::comment(
            format!("personal best for this mode: {best:.1} wpm"),
            palette,
        ),
        None => doc::blank(),
    }
}
