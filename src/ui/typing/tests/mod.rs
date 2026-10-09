mod code;
mod motion;

use std::time::Instant;

use code_racer_engine::{CodeLanguage, SessionOptions, TypingSession};
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::Disguise,
    config::{Look, Theme},
};

fn text_of(row: &Row) -> String {
    row.spans.iter().map(|span| span.content.as_ref()).collect()
}

fn view(session: &TypingSession, syntax: Option<CodeLanguage>) -> SessionView<'_> {
    SessionView {
        session,
        syntax,
        attribution: None,
        stopped_at: None,
        ink: None,
        disguise: None,
    }
}

fn still() -> Moment {
    Moment::still(Instant::now())
}

fn style_at(layout: &TypingLayout, row: usize, column: usize) -> Style {
    let mut start = 0;
    for span in &layout.rows[row].spans {
        let width = span.content.chars().count();
        if column < start + width {
            return span.style;
        }
        start += width;
    }
    panic!("no column {column} in row {row}");
}

#[test]
fn prose_rows_are_all_numbered() {
    let session = TypingSession::new("one two three four five six", SessionOptions::default());
    let layout = layout(
        &view(&session, None),
        10,
        &Palette::of(Theme::Editor),
        true,
        still(),
    );
    assert!(layout.rows.len() > 1);
    let numbers: Vec<Option<usize>> = layout.rows.iter().map(|row| row.number).collect();
    assert_eq!(numbers[1], Some(2));
}

#[test]
fn code_keeps_logical_line_numbers_and_shows_the_cursor_newline() {
    let mut session = TypingSession::new("fn a() {\n}", SessionOptions::default());
    let now = Instant::now();
    for ch in "fn a() {".chars() {
        session.type_char(ch, now);
    }
    let layout = layout(
        &view(&session, Some(CodeLanguage::Rust)),
        40,
        &Palette::of(Theme::Editor),
        true,
        still(),
    );
    assert_eq!(layout.rows.len(), 2);
    assert_eq!(layout.rows[1].number, Some(2));
    assert!(text_of(&layout.rows[0]).ends_with('↵'));
    assert_eq!(layout.cursor_row, 0);
}

#[test]
fn mistakes_show_the_expected_character() {
    let mut session = TypingSession::new("a b", SessionOptions::default());
    let now = Instant::now();
    session.type_char('a', now);
    session.type_char('x', now);
    let palette = Palette::of(Theme::Editor);
    let layout = layout(&view(&session, None), 20, &palette, true, still());
    let row = &layout.rows[0];
    assert_eq!(text_of(row), "a·b");
    let error = row
        .spans
        .iter()
        .find(|span| span.content == "·")
        .expect("error span");
    assert_eq!(error.style.fg, Some(palette.error));
}

#[test]
fn zero_width_characters_get_a_visible_placeholder() {
    let session = TypingSession::new("a\u{200b}b", SessionOptions::default());
    let layout = layout(
        &view(&session, None),
        20,
        &Palette::of(Theme::Editor),
        false,
        still(),
    );
    assert_eq!(text_of(&layout.rows[0]), "a◌b");
}

#[test]
fn identical_styles_are_merged_into_one_span() {
    let session = TypingSession::new("abcdef", SessionOptions::default());
    let layout = layout(
        &view(&session, None),
        20,
        &Palette::of(Theme::Editor),
        false,
        still(),
    );
    assert_eq!(layout.rows[0].spans.len(), 1);
}

#[test]
fn finished_text_shows_a_cursor_after_the_end_only_while_active() {
    let mut session = TypingSession::new("ab", SessionOptions::default());
    let now = Instant::now();
    session.type_char('a', now);
    session.type_char('b', now);
    let palette = Palette::of(Theme::Editor);
    assert_eq!(
        text_of(&layout(&view(&session, None), 20, &palette, false, still()).rows[0]),
        "ab"
    );
    let active = layout(&view(&session, None), 20, &palette, true, still());
    let row = &active.rows[0];
    assert_eq!(text_of(row), "ab ");
    let end = row.spans.last().expect("the cursor");
    assert_eq!((end.content.as_ref(), end.style), (" ", palette.cursor));
}

#[test]
fn a_look_dresses_prose_and_numbers_its_lines() {
    let session = TypingSession::new("ship the release notes", SessionOptions::default());
    let palette = Palette::of(Theme::Editor);
    let disguise = Disguise {
        look: Look::Todo,
        ..Disguise::default()
    };
    let view = SessionView {
        disguise: Some(disguise),
        ..view(&session, None)
    };
    let layout = layout(&view, 20, &palette, true, still());
    assert_eq!(text_of(&layout.rows[0]), "# TODO");
    assert!(text_of(&layout.rows[2]).starts_with("- [ ] ship"));
    assert_eq!(layout.cursor_row, 2, "the header comes first");
    let numbers: Vec<Option<usize>> = layout.rows.iter().map(|row| row.number).collect();
    assert_eq!(numbers[..3], [Some(1), Some(2), Some(3)]);
    assert!(
        layout
            .rows
            .iter()
            .skip(2)
            .all(|row| text_of(row).width() <= 20),
        "the checkboxes take room from the text"
    );
}

#[test]
fn code_ignores_the_look() {
    let session = TypingSession::new("let a = 1;", SessionOptions::default());
    let view = SessionView {
        disguise: Some(Disguise {
            look: Look::Todo,
            ..Disguise::default()
        }),
        ..view(&session, Some(CodeLanguage::Rust))
    };
    let layout = layout(&view, 40, &Palette::of(Theme::Editor), false, still());
    assert_eq!(layout.rows.len(), 1);
    assert_eq!(text_of(&layout.rows[0]), "let a = 1;");
}
