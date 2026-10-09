//! What prose looks like on screen: the kind of file it is typed in. A look
//! dresses the text with what such a file holds around it, before each row
//! and above and below, none of which is typed.

use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use code_racer_engine::{CodeLanguage, graphemes};
use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};
use unicode_normalization::UnicodeNormalization;
use unicode_width::UnicodeWidthStr;

use crate::{
    app::Disguise,
    config::Look,
    ui::{
        Moment,
        editor::Row,
        syntax,
        theme::{Palette, blend},
    },
};

/// How long the tick of a checklist row, or the stamp of a log row, glows.
const STAMP_GLOW: Duration = Duration::from_millis(700);
/// What stands for the time of a log row not typed yet.
const NO_TIME: &str = "··:··:··.···";
const LOG_LEVEL_WIDTH: usize = 5;

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
        Look::Todo => "- [ ] ".width(),
        Look::Docs => comment_marker(disguise.language).width(),
        Look::Log => NO_TIME.width() + 1 + LOG_LEVEL_WIDTH + 1,
    };
    u16::try_from(width).unwrap_or(0)
}

/// The lines of the file above the text.
pub fn header(disguise: Disguise<'_>, palette: &Palette) -> Vec<Row> {
    match disguise.look {
        Look::Todo => vec![title("# TODO", palette), Row::blank()],
        Look::Docs => docs_header(disguise.language)
            .iter()
            .map(|line| code(line, disguise.language, palette))
            .collect(),
        Look::Mail => {
            let author = author(disguise.author);
            let header = |key: &str, value: String| {
                Row::new(vec![
                    Span::styled(format!("{key}:"), palette.fg(palette.keyword)),
                    Span::styled(format!(" {value}"), palette.fg(palette.text)),
                ])
            };
            vec![
                header("From", format!("{author} <{}@localhost>", address(&author))),
                header("To", "team@lists.internal".to_owned()),
                header("Subject", "Re: notes from the sync".to_owned()),
                Row::blank(),
                plain("Hi all,", palette),
                Row::blank(),
            ]
        }
        Look::Notes | Look::Commit | Look::Log | Look::Shuffle => Vec::new(),
    }
}

/// The lines of the file below the text.
pub fn footer(disguise: Disguise<'_>, palette: &Palette) -> Vec<Row> {
    match disguise.look {
        Look::Commit => commit_template(palette),
        Look::Docs => docs_footer(disguise.language)
            .iter()
            .map(|line| code(line, disguise.language, palette))
            .collect(),
        Look::Mail => vec![
            Row::blank(),
            plain("Best,", palette),
            plain(&author(disguise.author), palette),
        ],
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
        Look::Todo => {
            let (mark, color) = if row.done {
                ("x", glowing(palette, palette.success, row.done_at, moment))
            } else {
                (" ", palette.muted)
            };
            vec![
                Span::styled("- [", palette.fg(palette.punctuation)),
                Span::styled(mark, palette.fg(color).add_modifier(Modifier::BOLD)),
                Span::styled("] ", palette.fg(palette.punctuation)),
            ]
        }
        Look::Docs => vec![Span::styled(
            comment_marker(disguise.language),
            palette.fg(palette.comment),
        )],
        Look::Log => {
            let level = log_level(row.index);
            let time = row.started.map_or_else(
                || Span::styled(NO_TIME, palette.fg(palette.faint)),
                |time| {
                    let color = glowing(palette, palette.number, row.started_at, moment);
                    Span::styled(time.format("%H:%M:%S%.3f").to_string(), palette.fg(color))
                },
            );
            let level_color = match level {
                "WARN" => palette.warning,
                "DEBUG" => palette.muted,
                _ => palette.success,
            };
            vec![
                time,
                Span::raw(" "),
                Span::styled(
                    format!("{level:<LOG_LEVEL_WIDTH$} "),
                    palette.fg(level_color),
                ),
            ]
        }
        Look::Notes | Look::Commit | Look::Mail | Look::Shuffle => Vec::new(),
    }
}

pub fn restyle(disguise: Disguise<'_>, row: RowState, style: Style, palette: &Palette) -> Style {
    match disguise.look {
        Look::Docs => style.add_modifier(Modifier::ITALIC),
        Look::Todo if row.done && !palette.mono => {
            style.fg(palette.muted).add_modifier(Modifier::CROSSED_OUT)
        }
        Look::Todo if row.done => style.add_modifier(Modifier::CROSSED_OUT),
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

/// Mostly information, now and then a debug line or a warning, always the
/// same for a given row.
fn log_level(index: usize) -> &'static str {
    const LEVELS: [&str; 10] = [
        "INFO", "INFO", "DEBUG", "INFO", "INFO", "WARN", "INFO", "DEBUG", "INFO", "INFO",
    ];
    LEVELS[index * 7 % LEVELS.len()]
}

fn comment_marker(language: CodeLanguage) -> &'static str {
    match language {
        CodeLanguage::Rust => "/// ",
        CodeLanguage::Python => "    ",
        CodeLanguage::TypeScript | CodeLanguage::JavaScript => " * ",
        CodeLanguage::Sql => "-- ",
    }
}

fn docs_header(language: CodeLanguage) -> &'static [&'static str] {
    match language {
        CodeLanguage::Rust | CodeLanguage::Sql => &[],
        CodeLanguage::Python => &["def reconcile(state: State) -> None:", "    \"\"\""],
        CodeLanguage::TypeScript | CodeLanguage::JavaScript => &["/**"],
    }
}

fn docs_footer(language: CodeLanguage) -> &'static [&'static str] {
    match language {
        CodeLanguage::Rust => &[
            "pub fn reconcile(state: &mut State) -> Result<(), Error> {",
            "    todo!()",
            "}",
        ],
        CodeLanguage::Python => &["    \"\"\"", "    raise NotImplementedError"],
        CodeLanguage::TypeScript => &[
            " */",
            "export function reconcile(state: State): void {",
            "  throw new Error(\"not implemented\");",
            "}",
        ],
        CodeLanguage::JavaScript => &[
            " */",
            "export function reconcile(state) {",
            "  throw new Error(\"not implemented\");",
            "}",
        ],
        CodeLanguage::Sql => &[
            "CREATE VIEW active_accounts AS",
            "SELECT id, email FROM accounts WHERE closed_at IS NULL;",
        ],
    }
}

/// The template git puts under a commit message.
fn commit_template(palette: &Palette) -> Vec<Row> {
    let comment = |text: &str| {
        Row::new(vec![Span::styled(
            text.to_owned(),
            palette.fg(palette.comment),
        )])
    };
    let file = |path: &str| {
        Row::new(vec![
            Span::styled("#       modified:   ", palette.fg(palette.comment)),
            Span::styled(path.to_owned(), palette.fg(palette.string)),
        ])
    };
    vec![
        Row::blank(),
        comment("# Please enter the commit message for your changes. Lines starting"),
        comment("# with '#' will be ignored, and an empty message aborts the commit."),
        comment("#"),
        Row::new(vec![
            Span::styled("# On branch ", palette.fg(palette.comment)),
            Span::styled("main", palette.fg(palette.function)),
        ]),
        comment("# Your branch is up to date with 'origin/main'."),
        comment("#"),
        comment("# Changes to be committed:"),
        file("src/lib.rs"),
        file("README.md"),
        comment("#"),
    ]
}

fn code(line: &str, language: CodeLanguage, palette: &Palette) -> Row {
    let text = graphemes(line);
    let tokens = syntax::highlight(&text, language);
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (grapheme, token) in text.iter().zip(tokens) {
        let style = palette.syntax(token);
        match spans.last_mut() {
            Some(last) if last.style == style => last.content.to_mut().push_str(grapheme),
            _ => spans.push(Span::styled(grapheme.clone(), style)),
        }
    }
    Row::new(spans)
}

fn title(text: &str, palette: &Palette) -> Row {
    Row::new(vec![Span::styled(
        text.to_owned(),
        palette.fg(palette.keyword).add_modifier(Modifier::BOLD),
    )])
}

fn plain(text: &str, palette: &Palette) -> Row {
    Row::new(vec![Span::styled(
        text.to_owned(),
        palette.fg(palette.text),
    )])
}

/// The name an email is signed with.
fn author(name: &str) -> String {
    match name.trim() {
        "" => "me".to_owned(),
        name => name.to_owned(),
    }
}

/// The local part of an email address for `name`: its letters without
/// their accents, a dot for each space.
fn address(name: &str) -> String {
    let address: String = name
        .to_lowercase()
        .nfd()
        .map(|ch| if ch.is_whitespace() { '.' } else { ch })
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        .collect();
    if address.is_empty() {
        "me".to_owned()
    } else {
        address
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let levels: Vec<&str> = (0..200).map(log_level).collect();
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
