use ratatui::style::Color;

use super::*;
use crate::config::Theme;

/// Relative luminance of a true colour, as WCAG defines it.
fn luminance(color: Color) -> Option<f64> {
    let Color::Rgb(red, green, blue) = color else {
        return None;
    };
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.039_28 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue))
}

/// WCAG contrast ratio of two true colours, from 1 to 21.
fn contrast(first: Color, second: Color) -> Option<f64> {
    let (first, second) = (luminance(first)?, luminance(second)?);
    Some((first.max(second) + 0.05) / (first.min(second) + 0.05))
}

#[test]
fn command_line_hints_are_readable_in_every_theme() {
    let mut app = app();
    for theme in Theme::ALL {
        app.config.theme = theme;
        let palette = Palette::of(theme);
        let terminal = drawn(&app, 100, 24, Instant::now());
        let hints = 23;
        let (key, _) = find(&terminal, "j/k move").expect("the hints");
        let key_style = style_at(&terminal, (key, hints));
        let action_style = style_at(&terminal, (key + 4, hints));
        let action = action_style.fg.expect("a colour");
        assert_eq!(key_style.fg, Some(palette.strong), "{theme}");
        assert!(key_style.add_modifier.contains(Modifier::BOLD), "{theme}");
        assert!(
            !action_style.add_modifier.contains(Modifier::DIM),
            "{theme}"
        );
        if !palette.mono {
            assert_ne!(action, palette.faint, "{theme}");
            assert_ne!(action, palette.muted, "{theme}");
        }
        if let Some(ratio) = contrast(action, palette.background) {
            assert!(ratio >= 4.5, "{theme}: {ratio:.2}");
        }
    }
}

#[test]
fn text_still_to_type_is_readable_on_the_cursor_line() {
    for theme in [Theme::Editor, Theme::VsCode] {
        let palette = Palette::of(theme);
        let pending = palette.pending.fg.expect("a colour");
        let line = palette.cursorline.bg.expect("a current line");
        for (place, background) in [("cursor line", line), ("buffer", palette.background)] {
            let ratio = contrast(pending, background).expect("true colours");
            assert!(ratio >= 4.5, "{theme} {place}: {ratio:.2}");
        }
        let typed = contrast(palette.strong, pending).expect("true colours");
        assert!(
            typed >= 2.0,
            "{theme}: typed text stands out from the rest: {typed:.2}"
        );
    }
}

#[test]
fn code_still_to_type_stays_readable_and_apart_from_typed_code() {
    use crate::ui::syntax::Token;
    let tokens = [
        Token::Keyword,
        Token::Type,
        Token::Function,
        Token::String,
        Token::Number,
        Token::Punctuation,
        Token::Plain,
    ];
    for theme in [Theme::Editor, Theme::VsCode] {
        let palette = Palette::of(theme);
        let ghost = palette.ghost.expect("syntax colours");
        let line = palette.cursorline.bg.expect("a current line");
        for token in tokens {
            let (dim, lit) = (ghost.of(token), palette.lit.of(token));
            let readable = contrast(dim, line).expect("true colours");
            assert!(readable >= 3.0, "{theme} {token:?}: {readable:.2}");
            let apart = contrast(lit, dim).expect("true colours");
            assert!(apart >= 1.6, "{theme} {token:?}: {apart:.2}");
        }
    }
}

#[test]
fn the_vscode_theme_has_a_blue_status_bar_and_no_tildes() {
    let mut app = app();
    app.config.theme = Theme::VsCode;
    let terminal = drawn(&app, 120, 30, Instant::now());
    let palette = Palette::of(Theme::VsCode);
    let status = 28;
    for column in [0, 60, 119] {
        let style = style_at(&terminal, (column, status));
        assert!(style.bg.is_some(), "column {column}: {style:?}");
    }
    assert_eq!(
        style_at(&terminal, (60, status)).bg,
        palette.status.bg,
        "the bar between its items"
    );
    assert_eq!(palette.status.bg, Some(Color::Rgb(0, 122, 204)));
    assert!(!text_of(&terminal).contains('~'), "{}", text_of(&terminal));
    let ratio = contrast(Color::Rgb(255, 255, 255), Color::Rgb(0, 122, 204)).expect("rgb");
    assert!(ratio >= 4.5, "white on the bar: {ratio:.2}");
}

#[test]
fn mono_tells_every_typing_state_apart_without_colour() {
    let palette = Palette::of(Theme::Mono);
    let styles = [
        ("cursor", palette.cursor),
        ("mistake", palette.mistake),
        ("pending", palette.pending),
        ("selection", palette.selection),
    ];
    for (name, style) in styles {
        assert_eq!(style.fg, None, "{name} uses no colour");
        assert_eq!(style.bg, None, "{name} uses no colour");
    }
    for (index, (name, style)) in styles.iter().enumerate() {
        for (other, other_style) in &styles[index + 1..] {
            assert_ne!(
                style.add_modifier, other_style.add_modifier,
                "{name} and {other} look the same"
            );
        }
    }
    assert!(!palette.mistake.add_modifier.contains(Modifier::REVERSED));
    assert!(palette.cursor.add_modifier.contains(Modifier::REVERSED));
}

#[test]
fn the_dark_theme_draws_on_black_with_visible_selections() {
    let mut app = app();
    app.config.theme = Theme::Dark;
    let palette = Palette::of(Theme::Dark);
    let terminal = drawn(&app, 120, 30, Instant::now());
    let blank = find(&terminal, "~").expect("an empty line");
    assert_eq!(style_at(&terminal, blank).bg, Some(Color::Black));
    let entry = find(&terminal, "   § practice.toml").expect("the explorer entry");
    let selected = style_at(&terminal, (entry.0 + 5, entry.1));
    assert_eq!(selected.bg, Some(palette.highlight));
    assert_ne!(palette.highlight, palette.background);

    press(&mut app, KeyCode::Enter);
    let terminal = drawn(&app, 120, 30, Instant::now());
    let line = find(&terminal, "mode ").expect("the selected line");
    let style = style_at(&terminal, line);
    assert!(style.add_modifier.contains(Modifier::BOLD), "{style:?}");
    assert_eq!(
        style.bg,
        Some(Color::Black),
        "no grey that hides ghost text"
    );
}

#[test]
fn the_mono_theme_shows_which_side_has_the_focus_without_colour() {
    let mut app = app();
    app.config.theme = Theme::Mono;
    let selected_entry = |app: &App| {
        let terminal = drawn(app, 120, 30, Instant::now());
        let entry = find(&terminal, "   § practice.toml").expect("the explorer entry");
        style_at(&terminal, (entry.0 + 5, entry.1))
    };
    let in_explorer = selected_entry(&app);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, crate::app::Focus::Editor);
    let in_editor = selected_entry(&app);
    assert!(
        in_explorer.add_modifier.contains(Modifier::REVERSED),
        "{in_explorer:?}"
    );
    assert_ne!(in_explorer, in_editor);
}
