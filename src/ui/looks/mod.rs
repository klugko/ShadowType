/*!
 * What prose looks like on screen: the kind of file it is typed in. A look
 * dresses the text with what such a file holds around it, before each row
 * and above and below, none of which is typed.
 */

mod commit;
mod docs;
mod log;
mod mail;
mod todo;

use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::Disguise,
    config::Look,
    ui::{
        Moment,
        editor::Row,
        theme::{Palette, blend},
    },
};

/// How long the tick of a checklist row, or the stamp of a log row, glows.
const STAMP_GLOW: Duration = Duration::from_millis(700);

/// The state of one row of the text, for what a look puts before it.
#[derive(Debug, Clone, Copy)]
pub struct RowState {
    pub index: usize,
    /// Whether every character of the row is typed right.
    pub done: bool,
    /// When the first character of the row was typed, by the wall clock.
    pub started: Option<DateTime<Local>>,
    /// When the first character of the row was typed.
    pub started_at: Option<Instant>,
    /// When the last character of the row was typed, once it is done.
    pub done_at: Option<Instant>,
}

/// Columns the look puts before every row of the text.
pub fn prefix_width(disguise: Disguise<'_>) -> u16 {
    let width = match disguise.look {
        Look::Notes | Look::Commit | Look::Mail | Look::Shuffle => 0,
        Look::Todo => todo::prefix_width(),
        Look::Docs => docs::comment_marker(disguise.language).width(),
        Look::Log => log::prefix_width(),
    };
    u16::try_from(width).unwrap_or(0)
}

/// The lines of the file above the text.
pub fn header(disguise: Disguise<'_>, palette: &Palette) -> Vec<Row> {
    match disguise.look {
        Look::Todo => todo::header(palette),
        Look::Docs => docs::header(disguise.language, palette),
        Look::Mail => mail::header(disguise.author, palette),
        Look::Notes | Look::Commit | Look::Log | Look::Shuffle => Vec::new(),
    }
}

/// The lines of the file below the text.
pub fn footer(disguise: Disguise<'_>, palette: &Palette) -> Vec<Row> {
    match disguise.look {
        Look::Commit => commit::footer(palette),
        Look::Docs => docs::footer(disguise.language, palette),
        Look::Mail => mail::footer(disguise.author, palette),
        Look::Notes | Look::Todo | Look::Log | Look::Shuffle => Vec::new(),
    }
}

pub fn prefix(
    disguise: Disguise<'_>,
    row: RowState,
    palette: &Palette,
    moment: Moment,
) -> Vec<Span<'static>> {
    match disguise.look {
        Look::Todo => todo::prefix(row, palette, moment),
        Look::Docs => vec![Span::styled(
            docs::comment_marker(disguise.language),
            palette.fg(palette.comment),
        )],
        Look::Log => log::prefix(row, palette, moment),
        Look::Notes | Look::Commit | Look::Mail | Look::Shuffle => Vec::new(),
    }
}

pub fn restyle(disguise: Disguise<'_>, row: RowState, style: Style, palette: &Palette) -> Style {
    match disguise.look {
        Look::Docs => style.add_modifier(Modifier::ITALIC),
        Look::Todo => todo::restyle(row, style, palette),
        _ => style,
    }
}

/// `color`, glowing from the glow colour of `palette` right after `since`.
fn glowing(palette: &Palette, color: Color, since: Option<Instant>, moment: Moment) -> Color {
    let (Some(glow), Some(since), true) = (palette.glow, since, moment.animate) else {
        return color;
    };
    let age = moment.now.saturating_duration_since(since);
    if age >= STAMP_GLOW {
        return color;
    }
    blend(glow, color, age.as_secs_f64() / STAMP_GLOW.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use code_racer_engine::CodeLanguage;

    use super::{log::NO_TIME, *};
    use crate::config::Theme;

    fn disguise(look: Look) -> Disguise<'static> {
        Disguise {
            look,
            language: CodeLanguage::Rust,
            author: "Élodie Dupont",
        }
    }

    fn text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn state(index: usize, done: bool) -> RowState {
        RowState {
            index,
            done,
            started: None,
            started_at: None,
            done_at: None,
        }
    }

    #[test]
    fn prefixes_take_the_width_they_announce() {
        let palette = Palette::of(Theme::Editor);
        let moment = Moment::still(Instant::now());
        for look in Look::ALL {
            for language in CodeLanguage::ALL {
                let disguise = Disguise {
                    language,
                    ..disguise(look)
                };
                let prefix = prefix(disguise, state(3, false), &palette, moment);
                assert_eq!(
                    text(&prefix).width(),
                    usize::from(prefix_width(disguise)),
                    "{look} in {language}"
                );
            }
        }
    }

    #[test]
    fn a_checklist_ticks_the_rows_that_are_done() {
        let palette = Palette::of(Theme::Editor);
        let moment = Moment::still(Instant::now());
        let todo = disguise(Look::Todo);
        assert_eq!(
            text(&prefix(todo, state(0, false), &palette, moment)),
            "- [ ] "
        );
        assert_eq!(
            text(&prefix(todo, state(0, true), &palette, moment)),
            "- [x] "
        );
        let done = restyle(todo, state(0, true), Style::new(), &palette);
        assert!(done.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn a_log_stamps_rows_once_typed() {
        let palette = Palette::of(Theme::Editor);
        let moment = Moment::still(Instant::now());
        let log = disguise(Look::Log);
        let waiting = text(&prefix(log, state(0, false), &palette, moment));
        assert!(waiting.starts_with(NO_TIME), "{waiting}");
        let typed = RowState {
            started: Local::now().with_time(chrono::NaiveTime::MIN).single(),
            ..state(0, false)
        };
        let stamped = text(&prefix(log, typed, &palette, moment));
        assert!(stamped.starts_with("00:00:00.000 "), "{stamped}");
    }

    #[test]
    fn logs_are_mostly_information() {
        let levels: Vec<&str> = (0..200).map(log::level).collect();
        let info = levels.iter().filter(|level| **level == "INFO").count();
        assert!(info > 100, "{info}");
        assert!(levels.contains(&"WARN") && levels.contains(&"DEBUG"));
        assert_eq!(levels[0], "INFO", "a log opens on information");
    }

    #[test]
    fn an_email_is_signed_by_the_player() {
        let palette = Palette::of(Theme::Editor);
        let mail = disguise(Look::Mail);
        let from = text(&header(mail, &palette)[0].spans);
        assert_eq!(from, "From: Élodie Dupont <elodie.dupont@localhost>");
        let footer = footer(mail, &palette);
        assert_eq!(text(&footer[footer.len() - 1].spans), "Élodie Dupont");
    }

    #[test]
    fn doc_comments_document_code_of_their_language() {
        let palette = Palette::of(Theme::Editor);
        let python = Disguise {
            language: CodeLanguage::Python,
            ..disguise(Look::Docs)
        };
        let header = header(python, &palette);
        assert_eq!(
            text(&header[0].spans),
            "def reconcile(state: State) -> None:"
        );
        let keyword = header[0].spans[0].style.fg;
        assert_eq!(keyword, Some(palette.keyword), "highlighted as code");
    }

    #[test]
    fn notes_add_nothing_to_the_text() {
        let palette = Palette::of(Theme::Editor);
        let notes = disguise(Look::Notes);
        assert_eq!(prefix_width(notes), 0);
        assert!(header(notes, &palette).is_empty());
        assert!(footer(notes, &palette).is_empty());
    }
}
