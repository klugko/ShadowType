//! Canonical form for every text a player is asked to type.

use unicode_normalization::UnicodeNormalization;

const TAB_WIDTH: usize = 4;

/// Normalises arbitrary text into something that can be typed on a keyboard.
///
/// Line endings become `\n`, tabs become spaces, trailing whitespace is removed
/// from every line, leading and trailing blank lines are dropped and the result
/// is converted to Unicode NFC so that composed and decomposed accents compare
/// equal.
pub fn normalize(text: &str) -> String {
    let lines: Vec<String> = text.lines().map(clean_line).collect();
    let first = lines.iter().position(|line| !line.is_empty());
    let last = lines.iter().rposition(|line| !line.is_empty());
    match (first, last) {
        (Some(first), Some(last)) => lines[first..=last].join("\n").nfc().collect(),
        _ => String::new(),
    }
}

fn clean_line(line: &str) -> String {
    line.replace('\t', &" ".repeat(TAB_WIDTH))
        .trim_end()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
