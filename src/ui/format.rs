/*!
 * How numbers, times and text are written on screen, in the columns
 * available.
 */

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::app::input::TextInput;

const ELLIPSIS: &str = "…";

/**
 * Completion as a whole percentage. It rounds to the nearest percent but
 * never shows 100 before the end: 99.6% of a text is not done.
 */
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
    let kept = head(text, width.saturating_sub(ELLIPSIS.width()));
    if width > 0 {
        format!("{kept}{ELLIPSIS}")
    } else {
        String::new()
    }
}

/// The part of a text field in view, and where its cursor is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputView {
    pub text: String,
    /// Column of the cursor in `text`.
    pub cursor: usize,
}

/**
 * What `input` shows in `width` columns. It scrolls as little as keeps
 * the text before the cursor in view with a column left for the cursor,
 * `…` standing for the start it hides. The end is cut at the edge.
 */
pub fn input_view(input: &TextInput, width: usize) -> InputView {
    let before = input.before_cursor();
    let after = &input.value()[before.len()..];
    let room = width.saturating_sub(1);
    let shown_before = if before.width() <= room {
        before.to_owned()
    } else {
        format!(
            "{ELLIPSIS}{}",
            tail(before, room.saturating_sub(ELLIPSIS.width()))
        )
    };
    InputView {
        cursor: shown_before.width(),
        text: head(&format!("{shown_before}{after}"), width).to_owned(),
    }
}

/// The longest start of `text` within `width` columns, whole graphemes only.
fn head(text: &str, width: usize) -> &str {
    let mut used = 0;
    for (index, grapheme) in text.grapheme_indices(true) {
        used += grapheme.width();
        if used > width {
            return &text[..index];
        }
    }
    text
}

/// The longest end of `text` within `width` columns, whole graphemes only.
fn tail(text: &str, width: usize) -> &str {
    let mut used = 0;
    for (index, grapheme) in text.grapheme_indices(true).rev() {
        used += grapheme.width();
        if used > width {
            return &text[index + grapheme.len()..];
        }
    }
    text
}

/**
 * `text` in a column of exactly `width` columns, cut short of the last one
 * so that it never touches the next column.
 */
pub fn column(text: &str, width: usize) -> String {
    let fitted = truncate(text, width.saturating_sub(1));
    let padding = width.saturating_sub(fitted.width());
    fitted + &" ".repeat(padding)
}

/**
 * The leading `parts` joined by `separator` that fit in `width` columns.
 * Trailing parts are dropped whole; the first part is cut only when it
 * does not fit alone.
 */
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
    use crossterm::event::{KeyCode, KeyEvent};

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

    /// A text field holding `before` and `after`, its cursor between them.
    fn input(before: &str, after: &str) -> TextInput {
        let mut input = TextInput::new(&format!("{before}{after}"), 200);
        for _ in after.graphemes(true) {
            input.handle_key(KeyEvent::from(KeyCode::Left));
        }
        input
    }

    fn view(text: &str, cursor: usize) -> InputView {
        InputView {
            text: text.to_owned(),
            cursor,
        }
    }

    #[test]
    fn a_field_that_fits_shows_whole() {
        assert_eq!(input_view(&input("join", ""), 10), view("join", 4));
        assert_eq!(input_view(&input("jo", "in"), 10), view("join", 2));
        assert_eq!(input_view(&input("", "join FK72AD"), 5), view("join ", 0));
    }

    #[test]
    fn a_long_field_scrolls_to_keep_its_cursor_in_view() {
        assert_eq!(
            input_view(&input("e src/main.rs", ""), 10),
            view("…/main.rs", 9),
            "a column is left for the cursor"
        );
        assert_eq!(
            input_view(&input("e src/main", ".rs"), 10),
            view("…src/main.", 9),
            "the end is cut"
        );
    }

    #[test]
    fn a_scrolled_field_never_splits_a_character() {
        assert_eq!(input_view(&input("a日本語", ""), 6), view("…本語", 5));
        assert_eq!(input_view(&input("ab日本", ""), 5), view("…本", 3));
        assert_eq!(input_view(&input("", "日本"), 3), view("日", 0));
        assert_eq!(
            input_view(&input("abce\u{301}", ""), 3),
            view("…e\u{301}", 2)
        );
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
