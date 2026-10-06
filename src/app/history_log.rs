//! Layout of the `history.log` buffer, shared by its view and by the keys
//! that scroll it, so that scrolling stops on its last line.

/// Sessions shown in the progression chart, the most recent ones.
pub const CHART_SESSIONS: usize = 60;
/// Rows of the progression chart.
pub const CHART_HEIGHT: u16 = 6;

/// The title and the summary: a comment, a blank line and four records.
const HEADER_LINES: usize = 7;
/// What an empty history shows: the title, a blank line and a hint.
const EMPTY_LINES: usize = 3;
/// A blank line and a heading above the chart.
const CHART_LINES: usize = 2 + CHART_HEIGHT as usize;
/// A blank line and the column names above the sessions.
const TABLE_HEADER_LINES: usize = 2;

/// Lines of the buffer for a history of `sessions` sessions. The chart needs
/// two sessions to draw a line.
pub fn line_count(sessions: usize) -> usize {
    match sessions {
        0 => EMPTY_LINES,
        1 => HEADER_LINES + TABLE_HEADER_LINES + 1,
        _ => HEADER_LINES + CHART_LINES + TABLE_HEADER_LINES + sessions,
    }
}
