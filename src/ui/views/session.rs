//! A solo session: the text while typing, then the results.

use std::time::Duration;

use code_racer_engine::Status;
use ratatui::{Frame, layout::Rect, style::Modifier, text::Span};

use super::doc;
use crate::{
    app::{
        App, SessionView,
        practice::{SoloResult, SoloRun},
    },
    ui::{
        Moment,
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

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    run: &SoloRun,
    palette: &Palette,
    moment: Moment,
) {
    match &run.result {
        Some(result) => results(frame, area, run, result, palette, moment),
        None => text(frame, area, app, palette, moment),
    }
}

/// How long the results take to come up: the figures count up and the
/// chart draws itself from left to right.
const REVEAL: Duration = Duration::from_millis(800);

/// How far the results are revealed at `moment`, from 0 to 1, eased.
fn revealed(result: &SoloResult, moment: Moment) -> f64 {
    if !moment.animate {
        return 1.0;
    }
    let age = moment.now.saturating_duration_since(result.at);
    let progress = (age.as_secs_f64() / REVEAL.as_secs_f64()).min(1.0);
    1.0 - (1.0 - progress).powi(3)
}

/// Whether the results are still coming up at `moment`.
pub fn reveals(result: &SoloResult, moment: Moment) -> bool {
    moment.animate && moment.now.saturating_duration_since(result.at) < REVEAL
}

fn text(frame: &mut Frame, area: Rect, app: &App, palette: &Palette, moment: Moment) {
    let Some(view) = app.session_view() else {
        return;
    };
    let (rows, cursor_row) = text_rows(&view, area, palette, app.is_typing(), moment);
    let scroll = editor::scroll_for(cursor_row, area.height, rows.len());
    editor::render(frame, area, &rows, scroll, palette);
}

/// Rows of a session text laid out for `area` at `moment`, and the row of
/// the cursor.
pub fn text_rows(
    view: &SessionView<'_>,
    area: Rect,
    palette: &Palette,
    active: bool,
    moment: Moment,
) -> (Vec<Row>, usize) {
    let mut highest = 999;
    loop {
        let width = editor::text_width(area.width, highest);
        let layout = typing::layout(view, width, palette, active, moment);
        if layout.rows.len() <= highest {
            return (layout.rows, layout.cursor_row);
        }
        highest = layout.rows.len();
    }
}

fn results(
    frame: &mut Frame,
    area: Rect,
    run: &SoloRun,
    result: &SoloResult,
    palette: &Palette,
    moment: Moment,
) {
    let reveal = revealed(result, moment);
    let stats = result.stats;
    let heading = match run.session().status() {
        Status::TimeUp => "time's up",
        _ => "session complete",
    };
    let mut rows = vec![
        doc::title(heading, palette),
        doc::comment(run.plan.label(), palette),
        doc::blank(),
        metric(
            "wpm",
            format!("{:.1}", stats.wpm * reveal),
            palette.fg(palette.number).add_modifier(Modifier::BOLD),
            palette,
        ),
        metric(
            "raw",
            format!("{:.1}", stats.raw_wpm * reveal),
            palette.fg(palette.number),
            palette,
        ),
        metric(
            "accuracy",
            format!("{:.1}%", stats.accuracy * reveal),
            palette.fg(palette.number),
            palette,
        ),
        metric(
            "errors",
            format!("{:.0}", stats.errors as f64 * reveal),
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
    ];
    if !result.missed.is_empty() {
        let mut table = missed_table(&result.missed, palette);
        let opening = table.remove(0);
        let mut spans = doc::assignment("missed", RESULT_KEY_WIDTH, opening, palette);
        spans.extend(table);
        rows.push(Row::new(spans));
    }
    rows.push(doc::blank());
    rows.push(personal_best(result, palette));
    rows.extend(comparison(result, palette));
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
        let mut chart = doc::chart(&wpm, width, height, palette);
        unveil(&mut chart, reveal);
        rows.extend(chart);
    }
    rows.extend(footer);
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

/// Hides the part of every row of `chart` past `reveal` of its width, for
/// it to draw itself from left to right.
fn unveil(chart: &mut [Row], reveal: f64) {
    if reveal >= 1.0 {
        return;
    }
    for row in chart {
        for span in &mut row.spans {
            let width = span.content.chars().count();
            let shown = (width as f64 * reveal).round() as usize;
            let text: String = span
                .content
                .chars()
                .enumerate()
                .map(|(column, ch)| if column < shown { ch } else { ' ' })
                .collect();
            span.content = text.into();
        }
    }
}

/// The characters missed most as a TOML inline table, such as
/// `{ e = 3, ";" = 1 }`.
fn missed_table(missed: &[(String, u32)], palette: &Palette) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled("{ ", palette.fg(palette.punctuation))];
    for (index, (expected, times)) in missed.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(", ", palette.fg(palette.punctuation)));
        }
        let bare = expected
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
        let key = match expected.as_str() {
            "\n" => r#""\n""#.to_owned(),
            "\"" => r#""\"""#.to_owned(),
            "\\" => r#""\\""#.to_owned(),
            _ if bare => expected.clone(),
            _ => format!("\"{expected}\""),
        };
        spans.push(Span::styled(key, palette.fg(palette.function)));
        spans.push(Span::styled(" = ", palette.fg(palette.punctuation)));
        spans.push(Span::styled(times.to_string(), palette.fg(palette.number)));
    }
    spans.push(Span::styled(" }", palette.fg(palette.punctuation)));
    spans
}

/// How the speed compares with the recent sessions before it, as a comment.
fn comparison(result: &SoloResult, palette: &Palette) -> Option<Row> {
    let recent = result.recent_wpm?;
    let difference = result.stats.wpm - recent;
    let text = if difference.abs() < 0.05 {
        format!("right on your recent average of {recent:.1} wpm")
    } else {
        let side = if difference > 0.0 { "above" } else { "below" };
        format!(
            "{:.1} wpm {side} your recent average of {recent:.1} wpm",
            difference.abs()
        )
    };
    Some(doc::comment(text, palette))
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
