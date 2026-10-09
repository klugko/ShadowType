use ratatui::style::Color;

use super::*;
use crate::config::Theme;

#[test]
fn typing_screen_shows_insert_mode_and_live_statistics() {
    let mut app = app();
    command(&mut app, "words 10");
    mistake(&mut app);
    let text = screen(&app, 100, 24);
    for expected in ["INSERT", "notes.md", "words 10", "-- INSERT --"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    let status = status_line(&text);
    for expected in [" - wpm ", " 0% ", " 1 error ", " 00:00 "] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
}

/**
 * The styles of a character typed right, one typed wrong, the cursor and a
 * character still to type, in that order on the cursor line, in `theme`.
 */
fn typing_cells(theme: Theme) -> [Style; 4] {
    let mut app = app();
    app.config.theme = theme;
    app.config.animations = false;
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    command(&mut app, "words 10");
    type_prefix(&mut app, 1);
    mistake(&mut app);
    let terminal = drawn(&app, MIN_WIDTH, MIN_HEIGHT, Instant::now());
    let first_row = 1;
    let text = 5;
    [0, 1, 2, 3].map(|offset| style_at(&terminal, (text + offset, first_row)))
}

#[test]
fn typed_text_shows_right_wrong_cursor_and_pending_characters_apart() {
    let palette = Palette::of(Theme::Editor);
    let [right, wrong, cursor, pending] = typing_cells(Theme::Editor);
    assert_eq!(right.fg, Some(palette.strong), "typed right");
    assert_eq!(wrong.fg, Some(palette.error), "typed wrong");
    assert!(wrong.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(
        (cursor.fg, cursor.bg),
        (Some(palette.on_accent), Some(palette.accent)),
        "the cursor"
    );
    assert_eq!(pending.fg, palette.pending.fg, "still to type");
    assert_ne!(pending.fg, right.fg);
    assert_eq!(pending.bg, Some(palette.highlight), "on the cursor line");
}

#[test]
fn the_mono_theme_shows_typed_text_apart_without_colour() {
    let cells = typing_cells(Theme::Mono);
    let [right, wrong, cursor, pending] = cells;
    for style in cells {
        for colour in [style.fg, style.bg] {
            assert!(matches!(colour, None | Some(Color::Reset)), "{style:?}");
        }
    }
    assert!(
        cursor.add_modifier.contains(Modifier::REVERSED),
        "{cursor:?}"
    );
    assert!(
        !wrong.add_modifier.contains(Modifier::REVERSED),
        "a mistake never looks like the cursor: {wrong:?}"
    );
    assert!(
        wrong.add_modifier.contains(Modifier::UNDERLINED),
        "{wrong:?}"
    );
    assert!(pending.add_modifier.contains(Modifier::DIM), "{pending:?}");
    assert!(
        !pending.add_modifier.contains(Modifier::BOLD),
        "no bold, which some terminals let win over dim: {pending:?}"
    );
    assert!(!right.add_modifier.contains(Modifier::DIM), "{right:?}");
}

#[test]
fn the_typing_view_scrolls_to_keep_context_above_the_cursor() {
    let directory = crate::persist::scratch::TempDir::new();
    let path = directory.join("lines.txt");
    let lines: Vec<String> = (1..=40).map(|line| format!("line {line:02}")).collect();
    std::fs::write(&path, lines.join("\n")).expect("write");
    let mut app = app();
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    command(&mut app, &format!("e {}", path.display()));
    type_prefix(&mut app, 25 * "line 01\n".len());

    let terminal = drawn(&app, MIN_WIDTH, MIN_HEIGHT, Instant::now());
    let text = text_of(&terminal);
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[1].starts_with(" 24  line 24"), "{text}");
    assert!(
        rows[3].starts_with(" 26  line 26"),
        "two lines of context:\n{text}"
    );
    let palette = Palette::of(Theme::Editor);
    assert_eq!(style_at(&terminal, (5, 3)).bg, Some(palette.accent));
    assert_eq!(style_at(&terminal, (1, 3)).fg, Some(palette.strong));
    assert_eq!(style_at(&terminal, (1, 2)).fg, Some(palette.faint));
}
