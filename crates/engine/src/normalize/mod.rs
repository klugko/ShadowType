mod keyboard;

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

pub(crate) use keyboard::keyboard_form;
use keyboard::{KeyboardForm, has_no_key};

const TAB_WIDTH: usize = 4;

/**
 * Every line ending other than `\r\n`: line feed, lone carriage return,
 * next line, and the Unicode line and paragraph separators.
 */
const LINE_BREAKS: [char; 5] = ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// The variation selector that shows the symbol before it as an emoji.
const EMOJI_PRESENTATION: char = '\u{fe0f}';

/**
 * Normalises arbitrary text into something that can be typed on a keyboard.
 *
 * - Every line ending becomes `\n`.
 * - Tabs become spaces up to the next multiple of four columns.
 * - Characters become what a keyboard types, as `keyboard_form` says:
 *   no-break and other spaces become a space, curly quotes and guillemets
 *   `'` or `"`, dashes `-`, box drawing `-`, `|` or `+`, and common symbols
 *   an ASCII spelling such as `...`, `->`, `<=` or `(c)`.
 * - Characters no key produces are dropped: byte order marks, zero-width
 *   and bidirectional formatting characters, the soft hyphen, control
 *   characters, and the symbols without a spelling that `has_no_key`
 *   lists. Letters and marks of every script, Latin-1 and currency symbols
 *   and emoji stay, with the zero-width joiner and variation selectors
 *   that emoji are made of.
 * - Trailing whitespace is removed from every line, leading and trailing
 *   blank lines are dropped, and the result is converted to Unicode NFC so
 *   that composed and decomposed accents compare equal.
 */
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

fn push_typeable(line: &mut String, ch: char) {
    match keyboard_form(ch) {
        KeyboardForm::Char(kept) if kept == ch && has_no_key(ch) => {}
        form => line.extend(form.chars()),
    }
}

/**
 * Replaces each tab with the spaces that reach the next tab stop, counting
 * columns in characters as the player sees them.
 */
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
mod tests;
