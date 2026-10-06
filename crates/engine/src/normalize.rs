//! Canonical form for every text a player is asked to type.

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const TAB_WIDTH: usize = 4;

/// Every line ending other than `\r\n`: line feed, lone carriage return,
/// next line, and the Unicode line and paragraph separators.
const LINE_BREAKS: [char; 5] = ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// Normalises arbitrary text into something that can be typed on a keyboard.
///
/// - Every line ending becomes `\n`.
/// - Tabs become spaces up to the next multiple of four columns.
/// - Typographic characters become what a keyboard types: no-break and other
///   spaces become a space, curly quotes and guillemets become `'` or `"`,
///   dashes become `-` and the ellipsis becomes `...`.
/// - Characters no key produces are dropped: byte order marks, zero-width
///   and bidirectional formatting characters, the soft hyphen and control
///   characters. The zero-width joiner and variation selectors stay, as they
///   belong to emoji.
/// - Trailing whitespace is removed from every line, leading and trailing
///   blank lines are dropped, and the result is converted to Unicode NFC so
///   that composed and decomposed accents compare equal.
pub fn normalize(text: &str) -> String {
    let lines: Vec<String> = text
        .split("\r\n")
        .flat_map(|chunk| chunk.split(LINE_BREAKS))
        .map(clean_line)
        .collect();
    let first = lines.iter().position(|line| !line.is_empty());
    let last = lines.iter().rposition(|line| !line.is_empty());
    match (first, last) {
        (Some(first), Some(last)) => lines[first..=last].join("\n").nfc().collect(),
        _ => String::new(),
    }
}

fn clean_line(line: &str) -> String {
    let mut typeable = String::with_capacity(line.len());
    for ch in line.chars() {
        push_typeable(&mut typeable, ch);
    }
    let mut cleaned = expand_tabs(&typeable);
    cleaned.truncate(cleaned.trim_end().len());
    cleaned
}

/// Appends what a keyboard types for `ch`, if anything. Tabs are kept for
/// [`expand_tabs`].
fn push_typeable(line: &mut String, ch: char) {
    match ch {
        '\t' => line.push(ch),
        '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{2032}' | '\u{2039}' | '\u{203a}' => {
            line.push('\'');
        }
        '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{2033}' | '\u{ab}' | '\u{bb}' => line.push('"'),
        '\u{2010}'..='\u{2015}' | '\u{2212}' => line.push('-'),
        '\u{2026}' => line.push_str("..."),
        ch if ch.is_control() || is_invisible(ch) => {}
        ch if ch.is_whitespace() => line.push(' '),
        ch => line.push(ch),
    }
}

/// Formatting characters that display as nothing and that no key produces.
fn is_invisible(ch: char) -> bool {
    matches!(
        ch,
        '\u{ad}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{200b}'
            | '\u{200c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
    )
}

/// Replaces each tab with the spaces that reach the next tab stop, counting
/// columns in characters as the player sees them.
fn expand_tabs(line: &str) -> String {
    let mut expanded = String::with_capacity(line.len());
    let mut column = 0;
    for grapheme in line.graphemes(true) {
        if grapheme == "\t" {
            let width = TAB_WIDTH - column % TAB_WIDTH;
            expanded.extend(std::iter::repeat_n(' ', width));
            column += width;
        } else {
            expanded.push_str(grapheme);
            column += 1;
        }
    }
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionOptions, Status, TypingSession};

    #[test]
    fn unifies_line_endings_and_whitespace() {
        let raw = "\r\n\nfn main() {\r\n\tprintln!();   \r\n}\r\n\r\n";
        assert_eq!(normalize(raw), "fn main() {\n    println!();\n}");
    }

    #[test]
    fn composes_decomposed_accents() {
        assert_eq!(normalize("e\u{301}te\u{301}"), "été");
    }

    #[test]
    fn blank_input_becomes_empty() {
        assert_eq!(normalize(" \n\t\n"), "");
    }

    #[test]
    fn inner_blank_lines_are_kept() {
        assert_eq!(normalize("a\n\n b"), "a\n\n b");
    }

    #[test]
    fn byte_order_marks_are_dropped() {
        assert_eq!(normalize("\u{feff}using System;\r\n"), "using System;");
        assert_eq!(normalize("a\u{feff}b"), "ab");
    }

    #[test]
    fn every_line_ending_becomes_a_newline() {
        assert_eq!(normalize("a\rb\r"), "a\nb");
        assert_eq!(
            normalize("a\u{85}b\u{2028}c\u{2029}d\r\ne"),
            "a\nb\nc\nd\ne"
        );
    }

    #[test]
    fn typographic_characters_become_ascii() {
        assert_eq!(normalize("l\u{2019}\u{e9}t\u{e9}"), "l'été");
        assert_eq!(
            normalize("\u{201C}oui\u{201D} \u{2018}non\u{2019}"),
            "\"oui\" 'non'"
        );
        assert_eq!(normalize("\u{ab}\u{a0}salut\u{a0}\u{bb}"), "\" salut \"");
        assert_eq!(normalize("a \u{2013} b \u{2014} c\u{2026}"), "a - b - c...");
        assert_eq!(normalize("Quoi\u{202f}?\u{2007}1"), "Quoi ? 1");
    }

    #[test]
    fn invisible_and_control_characters_are_dropped() {
        assert_eq!(normalize("a\u{200b}b\u{2060}c\u{ad}d"), "abcd");
        assert_eq!(normalize("\u{202e}abc\u{202c}\u{200f}"), "abc");
        assert_eq!(normalize("a\u{1b}[0mb\u{c}c\u{7f}"), "a[0mbc");
    }

    #[test]
    fn emoji_sequences_keep_their_joiners() {
        let emoji = "\u{1f469}\u{200d}\u{1f4bb} \u{2764}\u{fe0f}";
        assert_eq!(normalize(emoji), emoji);
    }

    #[test]
    fn tabs_reach_the_next_tab_stop() {
        assert_eq!(normalize("\tx"), "    x");
        assert_eq!(normalize("ab\tc"), "ab  c");
        assert_eq!(normalize("  \tx"), "    x");
        assert_eq!(normalize("abcd\te"), "abcd    e");
        assert_eq!(normalize("e\u{301}\tx"), "é   x");
    }

    #[test]
    fn only_typeable_characters_remain() {
        let raw = "\u{feff}\u{ab}\u{a0}\u{c7}a va\u{202f}? \u{bb}\r\n\tl\u{2019}\u{e9}t\u{e9}\u{2026}\u{200b}\u{1}\u{7f}\rfin\u{2028}";
        let normalized = normalize(raw);
        assert_eq!(normalized, "\" Ça va ? \"\n    l'été...\nfin");
        assert!(
            normalized
                .chars()
                .all(|ch| ch == '\n' || ch.is_ascii_graphic() || ch == ' ' || ch.is_alphabetic()),
            "{normalized:?}"
        );
    }

    #[test]
    fn a_normalised_windows_file_can_be_typed_to_the_end() {
        let now = std::time::Instant::now();
        let text = normalize("\u{feff}let x\u{a0}= 1;\r\n\tx\u{2026}\r\n");
        let mut session = TypingSession::new(&text, SessionOptions::default());
        for ch in "let x = 1;\n    x...".chars() {
            session.type_char(ch, now);
        }
        assert_eq!(session.status(), Status::Completed);
    }
}
