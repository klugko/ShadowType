use super::*;
use crate::ui::{syntax, typing::code::bracket_depths};

#[test]
fn code_still_to_type_shows_its_syntax_dimmed_and_lights_up_once_typed() {
    let mut session = TypingSession::new("let x = 1;\nlet y = 2;", SessionOptions::default());
    let now = Instant::now();
    for ch in "let x = 1;\n".chars() {
        session.type_char(ch, now);
    }
    let palette = Palette::of(Theme::Editor);
    let layout = layout(
        &view(&session, Some(CodeLanguage::Rust)),
        40,
        &palette,
        false,
        still(),
    );
    let ghost = palette.ghost.expect("true colours");
    let typed_keyword = style_at(&layout, 0, 0);
    let pending_keyword = style_at(&layout, 1, 0);
    let pending_number = style_at(&layout, 1, 8);
    assert_eq!(typed_keyword.fg, Some(palette.lit.keyword));
    assert_eq!(pending_keyword.fg, Some(ghost.keyword));
    assert_eq!(pending_number.fg, Some(ghost.number));
    assert_ne!(pending_keyword.fg, typed_keyword.fg);
}

#[test]
fn brackets_take_the_colour_of_their_depth() {
    let session = TypingSession::new("f(a[0])", SessionOptions::default());
    let palette = Palette::of(Theme::Editor);
    let layout = layout(
        &view(&session, Some(CodeLanguage::Rust)),
        40,
        &palette,
        false,
        still(),
    );
    let ghost = palette.ghost.expect("true colours");
    let outer = style_at(&layout, 0, 1);
    let inner = style_at(&layout, 0, 3);
    let inner_close = style_at(&layout, 0, 5);
    let outer_close = style_at(&layout, 0, 6);
    assert_eq!(outer.fg, Some(ghost.bracket(0)));
    assert_eq!(inner.fg, Some(ghost.bracket(1)));
    assert_eq!(inner_close.fg, inner.fg);
    assert_eq!(outer_close.fg, outer.fg);
}

#[test]
fn brackets_in_strings_are_not_counted() {
    let target: Vec<String> = "(\"(\")".chars().map(String::from).collect();
    let tokens = syntax::highlight(&target, CodeLanguage::Rust);
    let depths = bracket_depths(&target, &tokens);
    assert_eq!(depths, [Some(0), None, None, None, Some(0)]);
}

#[test]
fn indentation_shows_guides_at_each_level() {
    let text = "fn a() {\n    if b {\n        c();\n    }\n}";
    let session = TypingSession::new(text, SessionOptions::default());
    let palette = Palette::of(Theme::Editor);
    let layout = layout(
        &view(&session, Some(CodeLanguage::Rust)),
        40,
        &palette,
        false,
        still(),
    );
    assert_eq!(text_of(&layout.rows[1]), "│   if b { ");
    assert_eq!(text_of(&layout.rows[2]), "│   │   c(); ");
    assert_eq!(text_of(&layout.rows[0]), "fn a() { ");
}

#[test]
fn prose_has_no_indentation_guides() {
    let session = TypingSession::new("a    b", SessionOptions::default());
    let layout = layout(
        &view(&session, None),
        40,
        &Palette::of(Theme::Editor),
        false,
        still(),
    );
    assert_eq!(text_of(&layout.rows[0]), "a    b");
}
