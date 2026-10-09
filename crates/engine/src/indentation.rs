use crate::session::graphemes;

/**
 * The characters of a text that auto-indentation fills in: the spaces
 * opening every line but the first.
 *
 * A race server uses it to check how much of a player's progress was
 * filled in rather than typed.
 */
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Indentation {
    /// Grapheme positions, in increasing order.
    positions: Vec<usize>,
}

impl Indentation {
    pub fn of(text: &str) -> Self {
        let target = graphemes(text);
        let positions = target
            .iter()
            .enumerate()
            .filter(|(_, grapheme)| *grapheme == "\n")
            .flat_map(|(newline, _)| {
                let start = newline + 1;
                start..start + indentation_run(&target, start)
            })
            .collect();
        Self { positions }
    }

    /// How many of the first `typed` characters auto-indentation fills in.
    pub fn within(&self, typed: usize) -> usize {
        self.positions.partition_point(|&position| position < typed)
    }
}

/**
 * Number of spaces from `start` on: the indentation auto-indent fills in
 * when a newline was typed right before `start`.
 */
pub(crate) fn indentation_run(target: &[String], start: usize) -> usize {
    target.get(start..).map_or(0, |rest| {
        rest.iter().take_while(|grapheme| *grapheme == " ").count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        corpus,
        language::CodeLanguage,
        session::{Mark, SessionOptions, TypingSession},
    };

    #[test]
    fn leading_spaces_of_every_line_but_the_first_are_filled_in() {
        let indentation = Indentation::of("  a\n    b\n\n  c d");
        assert_eq!(indentation.within(3), 0, "the first line is typed by hand");
        assert_eq!(indentation.within(6), 2, "halfway through the indentation");
        assert_eq!(indentation.within(9), 4);
        assert_eq!(indentation.within(100), 6, "spaces inside a line are typed");
    }

    #[test]
    fn text_without_indentation_has_none() {
        assert_eq!(Indentation::of("one two\nthree").within(13), 0);
        assert_eq!(Indentation::of("").within(5), 0);
    }

    #[test]
    fn matches_what_a_session_fills_in_for_every_bundled_snippet() {
        let options = SessionOptions {
            auto_indent: true,
            ..SessionOptions::default()
        };
        let now = std::time::Instant::now();
        for language in CodeLanguage::ALL {
            for snippet in corpus::snippets(language) {
                let indentation = Indentation::of(snippet);
                let mut session = TypingSession::new(snippet, options);
                while let Some(expected) = session.target().get(session.cursor()).cloned() {
                    let before = session.cursor();
                    for ch in expected.chars() {
                        assert!(session.type_char(ch, now), "{ch:?} refused in {snippet}");
                    }
                    assert_eq!(session.mark(before), Mark::Correct, "{snippet}");
                    let tally = session.tally();
                    assert_eq!(
                        tally.indentation,
                        indentation.within(tally.typed),
                        "{snippet}"
                    );
                }
                assert!(session.is_finished(), "{snippet}");
            }
        }
    }
}
