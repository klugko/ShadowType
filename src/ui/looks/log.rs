use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::{RowState, glowing};
use crate::ui::{Moment, theme::Palette};

/// What stands for the time of a log row not typed yet.
pub(super) const NO_TIME: &str = "··:··:··.···";
const LEVEL_WIDTH: usize = 5;

pub(super) fn prefix_width() -> usize {
    NO_TIME.width() + 1 + LEVEL_WIDTH + 1
}

pub(super) fn prefix(row: RowState, palette: &Palette, moment: Moment) -> Vec<Span<'static>> {
    let level = level(row.index);
    let time = row.started.map_or_else(
        || Span::styled(NO_TIME, palette.fg(palette.faint)),
        |time| {
            let color = glowing(palette, palette.number, row.started_at, moment);
            Span::styled(time.format("%H:%M:%S%.3f").to_string(), palette.fg(color))
        },
    );
    let level_color = match level {
        "WARN" => palette.warning,
        "DEBUG" => palette.muted,
        _ => palette.success,
    };
    vec![
        time,
        Span::raw(" "),
        Span::styled(format!("{level:<LEVEL_WIDTH$} "), palette.fg(level_color)),
    ]
}

/**
 * Mostly information, now and then a debug line or a warning, always the
 * same for a given row.
 */
pub(super) fn level(index: usize) -> &'static str {
    const LEVELS: [&str; 10] = [
        "INFO", "INFO", "DEBUG", "INFO", "INFO", "WARN", "INFO", "DEBUG", "INFO", "INFO",
    ];
    LEVELS[index * 7 % LEVELS.len()]
}
