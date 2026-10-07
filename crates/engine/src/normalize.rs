//! Canonical form for every text a player is asked to type.

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const TAB_WIDTH: usize = 4;

/// Every line ending other than `\r\n`: line feed, lone carriage return,
/// next line, and the Unicode line and paragraph separators.
const LINE_BREAKS: [char; 5] = ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// The variation selector that shows the symbol before it as an emoji.
const EMOJI_PRESENTATION: char = '\u{fe0f}';

/// Normalises arbitrary text into something that can be typed on a keyboard.
///
/// - Every line ending becomes `\n`.
/// - Tabs become spaces up to the next multiple of four columns.
/// - Characters become what a keyboard types, as [`keyboard_form`] says:
///   no-break and other spaces become a space, curly quotes and guillemets
///   `'` or `"`, dashes `-`, box drawing `-`, `|` or `+`, and common symbols
///   an ASCII spelling such as `...`, `->`, `<=` or `(c)`.
/// - Characters no key produces are dropped: byte order marks, zero-width
///   and bidirectional formatting characters, the soft hyphen, control
///   characters, and the symbols without a spelling that [`has_no_key`]
///   lists. Letters and marks of every script, Latin-1 and currency symbols
///   and emoji stay, with the zero-width joiner and variation selectors
///   that emoji are made of.
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

/// What a keyboard types for one character, in the form texts take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyboardForm {
    /// The character itself or its plain equivalent, such as `'` for `’`.
    Char(char),
    /// An ASCII spelling of a symbol, such as `->` for `→`.
    Spelled(&'static str),
    /// Nothing: the character displays as nothing.
    Nothing,
}

impl KeyboardForm {
    fn chars(self) -> impl Iterator<Item = char> {
        let (single, spelled) = match self {
            Self::Char(ch) => (Some(ch), ""),
            Self::Spelled(spelling) => (None, spelling),
            Self::Nothing => (None, ""),
        };
        single.into_iter().chain(spelled.chars())
    }
}

/// Folds a character into what a keyboard types. Tabs and line feeds stay.
fn keyboard_form(ch: char) -> KeyboardForm {
    match ch {
        '\t' | '\n' => KeyboardForm::Char(ch),
        ch if ch.is_control() || is_invisible(ch) => KeyboardForm::Nothing,
        ch if ch.is_whitespace() => KeyboardForm::Char(' '),
        ch => plain_equivalent(ch)
            .map(KeyboardForm::Char)
            .or_else(|| ascii_spelling(ch).map(KeyboardForm::Spelled))
            .unwrap_or(KeyboardForm::Char(ch)),
    }
}

/// Folds the characters of a line, except emoji sequences, which stay whole.
fn clean_line(line: &str) -> String {
    let mut typeable = String::with_capacity(line.len());
    for grapheme in line.graphemes(true) {
        if grapheme.contains(EMOJI_PRESENTATION) {
            typeable.push_str(grapheme);
        } else {
            grapheme
                .chars()
                .for_each(|ch| push_typeable(&mut typeable, ch));
        }
    }
    let mut cleaned = expand_tabs(&typeable);
    cleaned.truncate(cleaned.trim_end().len());
    cleaned
}

/// Appends what a keyboard types for `ch`, if anything.
fn push_typeable(line: &mut String, ch: char) {
    match keyboard_form(ch) {
        KeyboardForm::Char(kept) if kept == ch && has_no_key(ch) => {}
        form => line.extend(form.chars()),
    }
}

/// The character a keyboard types in place of `ch`, when `ch` is a
/// typographic, compatibility or drawing variant of it.
fn plain_equivalent(ch: char) -> Option<char> {
    let plain = match ch {
        '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' | '\u{2032}' | '\u{2039}'
        | '\u{203a}' | '\u{b4}' | '\u{2bc}' => '\'',
        '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' | '\u{2033}' | '\u{ab}' | '\u{bb}' => '"',
        '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
        '\u{2022}' | '\u{2023}' | '\u{2043}' | '\u{2219}' | '\u{25e6}' => '*',
        '\u{d7}' => 'x',
        '\u{f7}' | '\u{2044}' | '\u{2215}' => '/',
        '\u{2500}'..='\u{257f}' => box_drawing_stroke(ch),
        '\u{2580}'..='\u{259f}' => '#',
        '\u{ff01}'..='\u{ff5e}' => fullwidth_ascii(ch)?,
        _ => return None,
    };
    Some(plain)
}

/// The ASCII stroke that draws a box-drawing character: `-` for horizontal
/// lines, `|` for vertical ones and `+` for corners and junctions.
fn box_drawing_stroke(ch: char) -> char {
    match ch {
        '─' | '━' | '┄' | '┅' | '┈' | '┉' | '═' | '╌' | '╍' | '╴' | '╶' | '╸' | '╺' | '╼' | '╾' => {
            '-'
        }
        '│' | '┃' | '┆' | '┇' | '┊' | '┋' | '║' | '╎' | '╏' | '╵' | '╷' | '╹' | '╻' | '╽' | '╿' => {
            '|'
        }
        '╱' => '/',
        '╲' => '\\',
        _ => '+',
    }
}

/// The ASCII character that a fullwidth form, as CJK input methods type
/// it, stands for.
fn fullwidth_ascii(ch: char) -> Option<char> {
    const OFFSET: u32 = '\u{ff01}' as u32 - '!' as u32;
    char::from_u32(u32::from(ch) - OFFSET)
}

/// How the symbols and ligatures that keyboards lack are spelled in ASCII.
fn ascii_spelling(ch: char) -> Option<&'static str> {
    let spelling = match ch {
        '\u{2026}' => "...",
        '→' => "->",
        '←' => "<-",
        '↔' => "<->",
        '⇒' => "=>",
        '⇐' => "<=",
        '⇔' => "<=>",
        '⟶' => "-->",
        '⟵' => "<--",
        '≤' => "<=",
        '≥' => ">=",
        '≠' => "!=",
        '≈' => "~=",
        '©' => "(c)",
        '®' => "(r)",
        '™' => "(tm)",
        'ﬀ' => "ff",
        'ﬁ' => "fi",
        'ﬂ' => "fl",
        'ﬃ' => "ffi",
        'ﬄ' => "ffl",
        'ﬅ' | 'ﬆ' => "st",
        _ => return None,
    };
    Some(spelling)
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

/// Symbols that no keyboard layout or input method types and that have no
/// ASCII spelling: the replacement characters left by lossy decoding,
/// private-use icons such as those of patched fonts, and the technical and
/// pictographic blocks from Arrows to Miscellaneous Symbols and Arrows.
/// Circled and parenthesised numbers and letters stay, as input methods
/// type them, and so do emoji.
fn has_no_key(ch: char) -> bool {
    match ch {
        '\u{fffc}' | '\u{fffd}' | '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{10ffff}' => true,
        '\u{2460}'..='\u{24ff}' => false,
        '\u{2190}'..='\u{2bff}' => !is_emoji(ch),
        _ => false,
    }
}

/// Symbols of the blocks [`has_no_key`] covers that are shown as emoji
/// without a presentation selector, as emoji pickers type them.
fn is_emoji(ch: char) -> bool {
    matches!(
        ch,
        '\u{231a}'..='\u{231b}'
            | '\u{23e9}'..='\u{23ec}'
            | '\u{23f0}'
            | '\u{23f3}'
            | '\u{25fd}'..='\u{25fe}'
            | '\u{2614}'..='\u{2615}'
            | '\u{2648}'..='\u{2653}'
            | '\u{267f}'
            | '\u{2693}'
            | '\u{26a1}'
            | '\u{26aa}'..='\u{26ab}'
            | '\u{26bd}'..='\u{26be}'
            | '\u{26c4}'..='\u{26c5}'
            | '\u{26ce}'
            | '\u{26d4}'
            | '\u{26ea}'
            | '\u{26f2}'..='\u{26f3}'
            | '\u{26f5}'
            | '\u{26fa}'
            | '\u{26fd}'
            | '\u{2705}'
            | '\u{270a}'..='\u{270b}'
            | '\u{2728}'
            | '\u{274c}'
            | '\u{274e}'
            | '\u{2753}'..='\u{2755}'
            | '\u{2757}'
            | '\u{2795}'..='\u{2797}'
            | '\u{27b0}'
            | '\u{27bf}'
            | '\u{2b1b}'..='\u{2b1c}'
            | '\u{2b50}'
            | '\u{2b55}'
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
    fn common_symbols_take_their_ascii_spelling() {
        assert_eq!(
            normalize("a \u{2192} b \u{2190} c \u{2194} d \u{21d2} e \u{21d0} f \u{27f6} g"),
            "a -> b <- c <-> d => e <= f --> g"
        );
        assert_eq!(
            normalize("x \u{2264} y \u{2265} z \u{2260} w \u{2248} v"),
            "x <= y >= z != w ~= v"
        );
        assert_eq!(
            normalize("\u{2022} a \u{2023} b \u{25e6} c 2 \u{d7} 3 \u{f7} 4"),
            "* a * b * c 2 x 3 / 4"
        );
        assert_eq!(normalize("\u{a9} \u{ae} \u{2122}"), "(c) (r) (tm)");
        assert_eq!(
            normalize("\u{201b}a\u{2019} \u{201f}b\u{201d} it\u{b4}s don\u{2bc}t"),
            "'a' \"b\" it's don't"
        );
    }

    #[test]
    fn box_drawing_becomes_ascii_art() {
        let raw = "// \u{2500}\u{2500} x \u{2550}\u{2550}\n\u{250c}\u{2500}\u{256e}\n\u{2502}a\u{2551}\n\u{2514}\u{2501}\u{256f}\n\u{2580}\u{2584}\u{2588}\u{2591}";
        assert_eq!(normalize(raw), "// -- x --\n+-+\n|a|\n+-+\n####");
    }

    #[test]
    fn compatibility_forms_become_plain_letters() {
        assert_eq!(
            normalize("\u{fb01}n \u{fb02}ux \u{ff32}\u{ff55}\u{ff53}\u{ff54}\u{ff01}"),
            "fin flux Rust!"
        );
    }

    #[test]
    fn symbols_no_key_types_are_dropped() {
        assert_eq!(normalize("done \u{2713}"), "done");
        assert_eq!(normalize("a\u{fffd}b"), "ab");
        assert_eq!(normalize("\u{e0b0} main"), " main");
        assert_eq!(normalize("\u{2318}S \u{221e}"), "S");
    }

    #[test]
    fn symbols_of_european_keyboards_stay() {
        let symbols =
            "\u{20ac} \u{a3} \u{a7} \u{b0} \u{b2} \u{b5} \u{a4} \u{bf} \u{a1} l\u{b7}l \u{bd}";
        assert_eq!(normalize(symbols), symbols);
    }

    #[test]
    fn emoji_stay_whether_or_not_they_carry_a_presentation_selector() {
        let emoji = "\u{2705} \u{2728} \u{2b50} \u{2764}\u{fe0f} \u{2714}\u{fe0f} \u{1f44d}";
        assert_eq!(normalize(emoji), emoji);
    }

    #[test]
    fn a_file_full_of_symbols_normalises_to_typeable_characters() {
        let raw = "/// a \u{2192} b \u{2022} c \u{2713} \u{d7} \u{2264} \u{2500}\u{2502} \u{a9} \u{2122} \u{b4} \u{2bc} \u{201b} \u{201f} \u{fffd} \u{2318} \u{25a0} \u{2605} \u{2800} \u{fb01}";
        let normalized = normalize(raw);
        assert!(
            normalized.chars().all(|ch| ch == ' '
                || ch.is_ascii_graphic()
                || ch.is_alphabetic()
                || ('\u{a1}'..='\u{ff}').contains(&ch)),
            "{normalized:?}"
        );
    }

    #[test]
    fn a_normalised_file_with_symbols_can_be_typed_to_the_end() {
        let now = std::time::Instant::now();
        let text = normalize(
            "// \u{2500}\u{2500} x \u{2500}\u{2500}\nlet b = a \u{2192} c; // \u{2264} \u{2713}",
        );
        let mut session = TypingSession::new(&text, SessionOptions::default());
        for ch in "// -- x --\nlet b = a -> c; // <=".chars() {
            session.type_char(ch, now);
        }
        assert_eq!(session.status(), Status::Completed);
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
