//! How numbers, times and text are written on screen, in the columns
//! available.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const ELLIPSIS: &str = "…";

/// Completion as a whole percentage. It rounds to the nearest percent but
/// never shows 100 before the end: 99.6% of a text is not done.
pub fn percent_done(fraction: f64) -> u32 {
    let percent = (fraction.clamp(0.0, 1.0) * 100.0).round() as u32;
    if fraction < 1.0 { percent.min(99) } else { 100 }
}

/// `mm:ss` clock.
pub fn clock(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// `m:ss.t` race time.
pub fn race_time(milliseconds: u64) -> String {
    let tenths = milliseconds / 100;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// `1st`, `2nd`, `3rd`, `4th`…
pub fn ordinal(place: usize) -> String {
    let suffix = match (place % 10, place % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{place}{suffix}")
}

/// `text` within `width` columns, ending with `…` when it had to be cut.
pub fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let room = width.saturating_sub(ELLIPSIS.width());
    let mut kept = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        used += grapheme.width();
        if used > room {
            break;
        }
        kept.push_str(grapheme);
    }
    if width > 0 {
        kept.push_str(ELLIPSIS);
    }
    kept
}

/// `text` in a column of exactly `width` columns, cut short of the last one
/// so that it never touches the next column.
pub fn column(text: &str, width: usize) -> String {
    let fitted = truncate(text, width.saturating_sub(1));
    let padding = width.saturating_sub(fitted.width());
    fitted + &" ".repeat(padding)
}

/// The leading `parts` joined by `separator` that fit in `width` columns.
/// Trailing parts are dropped whole; the first part is cut only when it
/// does not fit alone.
pub fn leading_parts(parts: &[String], separator: &str, width: usize) -> String {
    let mut joined = String::new();
    for part in parts {
        let candidate = if joined.is_empty() {
            part.clone()
        } else {
            format!("{joined}{separator}{part}")
        };
        if candidate.width() > width {
            break;
        }
        joined = candidate;
    }
    match parts.first() {
        Some(first) if joined.is_empty() => truncate(first, width),
        _ => joined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_reaches_100_only_at_the_end() {
        assert_eq!(percent_done(0.0), 0);
        assert_eq!(percent_done(0.294), 29);
        assert_eq!(percent_done(0.296), 30);
        assert_eq!(percent_done(0.996), 99);
        assert_eq!(percent_done(0.999_999), 99);
        assert_eq!(percent_done(1.0), 100);
    }

    #[test]
    fn clocks_use_minutes_and_seconds() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(754), "12:34");
    }

    #[test]
    fn race_times_use_minutes_seconds_and_tenths() {
        assert_eq!(race_time(41_234), "0:41.2");
        assert_eq!(race_time(61_000), "1:01.0");
    }

    #[test]
    fn ordinals_handle_the_teens() {
        let places: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 101].map(ordinal).to_vec();
        assert_eq!(
            places,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "101st"
            ]
        );
    }

    #[test]
    fn long_text_is_cut_with_an_ellipsis() {
        assert_eq!(truncate("notes.md", 8), "notes.md");
        assert_eq!(truncate("a-very-long-name.rs", 8), "a-very-…");
        assert_eq!(truncate("日本語のファイル", 7), "日本語…");
        assert_eq!(truncate("abc", 1), "…");
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn columns_have_an_exact_width_and_a_gap() {
        assert_eq!(column("jean", 8), "jean    ");
        assert_eq!(column("a-very-long-name", 8), "a-very… ");
        assert_eq!(column("été", 5), "été  ");
        assert_eq!(column("日本語のファイル", 8).width(), 8);
    }

    #[test]
    fn trailing_parts_are_dropped_before_the_first_is_cut() {
        let parts = ["words 100", "english", "punctuation", "numbers"].map(String::from);
        assert_eq!(
            leading_parts(&parts, " · ", 100),
            "words 100 · english · punctuation · numbers"
        );
        assert_eq!(leading_parts(&parts, " · ", 25), "words 100 · english");
        assert_eq!(leading_parts(&parts, " · ", 9), "words 100");
        assert_eq!(leading_parts(&parts, " · ", 6), "words…");
        assert_eq!(leading_parts(&[], " · ", 6), "");
    }
}
