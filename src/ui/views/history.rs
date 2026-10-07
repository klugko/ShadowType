//! `history.log`: personal records, a progression chart and past sessions.

use std::ops::Range;

use ratatui::{Frame, layout::Rect, style::Modifier, text::Span};

use super::doc;
use crate::{
    app::{
        App,
        history_log::{self, CHART_HEIGHT, CHART_SESSIONS, Figure, Line},
    },
    history::{Record, Summary},
    ui::{
        editor::{self, Row},
        theme::Palette,
    },
};

/// Draws the lines of [`history_log::lines`] in view. The lines out of view
/// are left empty: only their numbers count, for the width of the gutter.
pub fn render(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) {
    let records = app.history.records();
    let lines = history_log::lines(records.len());
    let page = Page {
        records,
        summary: app.history.summary(),
        chart: chart(
            records,
            editor::text_width(area.width, lines.len()),
            palette,
        ),
        palette,
    };
    let in_view: Range<usize> = app.history_scroll..app.history_scroll + usize::from(area.height);
    let mut rows: Vec<Row> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if in_view.contains(&index) {
                page.row(*line)
            } else {
                Row::blank()
            }
        })
        .collect();
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, app.history_scroll, palette);
}

/// What the lines of the buffer are drawn from.
struct Page<'a> {
    records: &'a [Record],
    summary: Summary,
    chart: Vec<Row>,
    palette: &'a Palette,
}

impl Page<'_> {
    fn row(&self, line: Line) -> Row {
        let palette = self.palette;
        match line {
            Line::Title => doc::title("history.log", palette),
            Line::Blank => doc::blank(),
            Line::NoSessions => doc::comment("no sessions yet, press s to start one", palette),
            Line::Totals => doc::comment(
                format!(
                    "{} sessions, {} of practice",
                    self.summary.sessions,
                    duration(self.summary.total_time.as_secs())
                ),
                palette,
            ),
            Line::Figure(figure) => figure_row(figure, &self.summary, palette),
            Line::ChartHeading { sessions } => {
                doc::heading(format!("wpm, last {sessions} sessions"), palette)
            }
            Line::Chart(row) => self
                .chart
                .get(usize::from(row))
                .cloned()
                .unwrap_or_default(),
            Line::Columns => Row::new(vec![Span::styled(
                format!(
                    "{:<17}{:<12}{:<12}{:>6}{:>6}{:>8}{:>5}",
                    "date", "mode", "language", "wpm", "raw", "acc", "err"
                ),
                palette.fg(palette.muted).add_modifier(Modifier::BOLD),
            )]),
            Line::Session(rank) => self
                .records
                .len()
                .checked_sub(rank + 1)
                .and_then(|index| self.records.get(index))
                .map(|record| session_row(record, palette))
                .unwrap_or_default(),
        }
    }
}

/// The rows of the progression chart of the last sessions, in a text
/// `width` columns wide, none before two sessions.
fn chart(records: &[Record], width: u16, palette: &Palette) -> Vec<Row> {
    let recent: Vec<f64> = records
        .iter()
        .rev()
        .take(CHART_SESSIONS)
        .rev()
        .map(|record| record.wpm)
        .collect();
    if recent.len() < 2 {
        return Vec::new();
    }
    doc::chart(&recent, width, CHART_HEIGHT, palette)
}

fn session_row(record: &Record, palette: &Palette) -> Row {
    Row::new(vec![
        Span::styled(
            format!("{:<17}", record.date.format("%Y-%m-%d %H:%M")),
            palette.fg(palette.comment),
        ),
        Span::styled(format!("{:<12}", record.mode), palette.fg(palette.function)),
        Span::styled(format!("{:<12}", record.language), palette.fg(palette.text)),
        Span::styled(format!("{:>6.0}", record.wpm), palette.fg(palette.number)),
        Span::styled(
            format!("{:>6.0}", record.raw_wpm),
            palette.fg(palette.muted),
        ),
        Span::styled(
            format!("{:>7.1}%", record.accuracy),
            palette.fg(palette.number),
        ),
        Span::styled(format!("{:>5}", record.errors), palette.fg(palette.text)),
    ])
}

fn figure_row(figure: Figure, summary: &Summary, palette: &Palette) -> Row {
    let (key, value, comment) = match figure {
        Figure::BestWpm => ("best_wpm", summary.best_wpm, ""),
        Figure::AverageWpm => ("average_wpm", summary.average_wpm, ""),
        Figure::RecentWpm => ("recent_wpm", summary.recent_wpm, "last 10 sessions"),
        Figure::BestAccuracy => ("best_accuracy", summary.best_accuracy, "percent"),
    };
    let value = Span::styled(format!("{value:.1}"), palette.fg(palette.number));
    let mut spans = doc::assignment(key, 13, value, palette);
    if !comment.is_empty() {
        spans.push(Span::styled(
            format!("  # {comment}"),
            palette.fg(palette.comment),
        ));
    }
    Row::new(spans)
}

/// `1h 05m`, `12m 30s` or `45s`.
fn duration(seconds: u64) -> String {
    match (seconds / 3600, seconds / 60 % 60, seconds % 60) {
        (0, 0, seconds) => format!("{seconds}s"),
        (0, minutes, seconds) => format!("{minutes}m {seconds:02}s"),
        (hours, minutes, _) => format!("{hours}h {minutes:02}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_pick_the_two_largest_units() {
        assert_eq!(duration(45), "45s");
        assert_eq!(duration(750), "12m 30s");
        assert_eq!(duration(3_900), "1h 05m");
    }
}
