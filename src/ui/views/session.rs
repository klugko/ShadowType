//! A solo session: the text while typing, then the results.

use code_racer_engine::Status;
use ratatui::{Frame, layout::Rect, style::Modifier, text::Span};

use super::{doc, race_time};
use crate::{
    app::{
        App, SessionView,
        practice::{SoloResult, SoloRun},
    },
    ui::{
        chart,
        editor::{self, Row},
        theme::Palette,
        typing,
    },
};

const RESULT_KEY_WIDTH: usize = 11;
/// Columns taken by the y-axis labels of a chart.
const CHART_LABEL_WIDTH: usize = 8;

pub fn render(frame: &mut Frame, area: Rect, app: &App, run: &SoloRun, palette: &Palette) {
    match &run.result {
        Some(result) => results(frame, area, run, result, palette),
        None => text(frame, area, app, run, palette),
    }
}

fn text(frame: &mut Frame, area: Rect, app: &App, run: &SoloRun, palette: &Palette) {
    let view = SessionView {
        session: &run.session,
        syntax: run.plan.syntax(),
        attribution: run.attribution.as_deref(),
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
    let room = usize::from(area.height).saturating_sub(rows.len() + footer.len() + 2);
    if wpm.len() >= 2 && room >= 4 {
        rows.push(doc::blank());
        rows.push(doc::heading("wpm over time", palette));
        let width = editor::text_width(area.width, 99).saturating_sub(2);
        let height = u16::try_from(room.min(8)).unwrap_or(4);
        rows.extend(chart_rows(&wpm, width, height, palette));
    }
    rows.extend(footer);
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

pub fn chart_rows(values: &[f64], width: u16, height: u16, palette: &Palette) -> Vec<Row> {
    let plot_width = usize::from(width).saturating_sub(CHART_LABEL_WIDTH);
    chart::line_chart(&stretch(values, plot_width), width, height)
        .into_iter()
        .map(|line| Row::new(vec![Span::styled(line, palette.fg(palette.accent))]))
        .collect()
}

/// Interpolates a short series so that it spans `columns` columns.
fn stretch(values: &[f64], columns: usize) -> Vec<f64> {
    if values.len() < 2 || values.len() >= columns {
        return values.to_vec();
    }
    let last = (values.len() - 1) as f64;
    (0..columns)
        .map(|column| {
            let position = column as f64 * last / (columns - 1) as f64;
            let index = position.floor() as usize;
            let next = (index + 1).min(values.len() - 1);
            let fraction = position - index as f64;
            values[index] + (values[next] - values[index]) * fraction
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretch_keeps_the_endpoints_and_interpolates_between_them() {
        let stretched = stretch(&[10.0, 20.0], 5);
        assert_eq!(stretched, [10.0, 12.5, 15.0, 17.5, 20.0]);
    }

    #[test]
    fn stretch_leaves_long_series_alone() {
        let values = [1.0, 2.0, 3.0];
        assert_eq!(stretch(&values, 2), values);
        assert_eq!(stretch(&[4.0], 10), [4.0]);
    }
}
