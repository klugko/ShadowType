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
