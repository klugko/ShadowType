//! `history.log`: personal records, a progression chart and past sessions.

use ratatui::{Frame, layout::Rect, style::Modifier, text::Span};

use super::{doc, session::chart_rows};
use crate::{
    app::App,
    history::Summary,
    ui::{
        editor::{self, Row},
        theme::Palette,
    },
};

const CHART_SESSIONS: usize = 60;
const CHART_HEIGHT: u16 = 6;

pub fn render(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) {
    let records = app.history.records();
    let mut rows = vec![doc::title("history.log", palette)];
    if records.is_empty() {
        rows.push(doc::blank());
        rows.push(doc::comment(
            "no sessions yet, press s to start one",
            palette,
        ));
        editor::number_rows(&mut rows);
        editor::render(frame, area, &rows, 0, palette);
        return;
    }
    rows.extend(summary_rows(&app.history.summary(), palette));
    let recent: Vec<f64> = records
        .iter()
        .rev()
        .take(CHART_SESSIONS)
        .rev()
        .map(|record| record.wpm)
        .collect();
    if recent.len() >= 2 {
        rows.push(doc::blank());
        rows.push(doc::heading(
            format!("wpm, last {} sessions", recent.len()),
            palette,
        ));
        let width = editor::text_width(area.width, 999).saturating_sub(2);
        rows.extend(chart_rows(&recent, width, CHART_HEIGHT, palette));
    }
    rows.push(doc::blank());
    rows.push(Row::new(vec![Span::styled(
        format!(
            "{:<17}{:<12}{:<12}{:>6}{:>6}{:>8}{:>5}",
            "date", "mode", "language", "wpm", "raw", "acc", "err"
        ),
        palette.fg(palette.muted).add_modifier(Modifier::BOLD),
    )]));
    rows.extend(records.iter().rev().map(|record| {
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
    }));
    editor::number_rows(&mut rows);
    let scroll = app.history_scroll.min(rows.len().saturating_sub(1));
    editor::render(frame, area, &rows, scroll, palette);
}

fn summary_rows(summary: &Summary, palette: &Palette) -> Vec<Row> {
    let number = |value: String| Span::styled(value, palette.fg(palette.number));
    vec![
        doc::comment(
            format!(
                "{} sessions, {} of practice",
                summary.sessions,
                duration(summary.total_time.as_secs())
            ),
            palette,
        ),
        doc::blank(),
        summary_row(
            "best_wpm",
            number(format!("{:.1}", summary.best_wpm)),
            "",
            palette,
        ),
        summary_row(
            "average_wpm",
            number(format!("{:.1}", summary.average_wpm)),
            "",
            palette,
        ),
        summary_row(
            "recent_wpm",
            number(format!("{:.1}", summary.recent_wpm)),
            "last 10 sessions",
            palette,
        ),
        summary_row(
            "best_accuracy",
            number(format!("{:.1}", summary.best_accuracy)),
            "percent",
            palette,
        ),
    ]
}

fn summary_row(key: &str, value: Span<'static>, comment: &str, palette: &Palette) -> Row {
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
