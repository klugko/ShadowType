use crate::config::Theme;

use super::*;

#[test]
fn home_looks_like_an_editor() {
    let text = screen(&app(), 100, 30);
    for expected in [
        "EXPLORER",
        "practice.toml",
        "race.toml",
        "history.log",
        "NORMAL",
        "mode",
        "words",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[test]
fn small_terminals_get_a_message_instead_of_a_broken_layout() {
    let app = app();
    let text = screen(&app, MIN_WIDTH - 1, MIN_HEIGHT);
    assert!(text.contains("Terminal too small."), "{text}");
    assert!(text.contains("80x20"));
    for (width, height) in [(1, 1), (10, 4), (MIN_WIDTH, MIN_HEIGHT - 1)] {
        screen(&app, width, height);
    }
    assert!(!screen(&app, MIN_WIDTH, MIN_HEIGHT).contains("too small"));
}

#[test]
fn the_smallest_size_gives_the_explorer_columns_to_whole_buffer_lines() {
    let mut app = app();
    for wpm in [72.0, 81.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let home = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    assert!(!home.contains("EXPLORER"), "{home}");
    assert!(home.contains("# 10 · 25 · 50 · 100"), "{home}");
    for (page, line) in [
        ("history", "acc  err"),
        ("help", "edit a text value, Enter saves, Esc cancels"),
        ("config", "# e.g. ws://192.168.1.42:8080"),
    ] {
        command(&mut app, page);
        let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
        assert!(text.contains(line), "{page}:\n{text}");
    }
}

#[test]
fn every_buffer_renders_in_every_theme_and_size() {
    let mut app = app();
    app.history.add(record(72.0)).expect("in memory");
    app.history.add(record(81.0)).expect("in memory");
    for theme in Theme::ALL {
        app.config.theme = theme;
        for page in [
            "practice",
            "race",
            "history",
            "config",
            "help",
            "words 25",
            "code rust",
        ] {
            command(&mut app, page);
            for (width, height) in [(80, 20), (120, 40), (200, 60)] {
                for sidebar in [true, false] {
                    app.sidebar = sidebar;
                    screen(&app, width, height);
                }
            }
            press(&mut app, KeyCode::Esc);
        }
    }
}

#[test]
fn files_have_icons_in_the_explorer_and_the_tabs() {
    use crate::config::Icons;
    let mut app = app();
    command(&mut app, "set look=commit");
    command(&mut app, "words 10");
    type_prefix(&mut app, 1);
    let text = screen(&app, 120, 30);
    for expected in [
        "§ practice.toml",
        "¶ help.md",
        "≡ history.log",
        "± COMMIT_EDITMSG",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    let tab = text.lines().next().unwrap_or_default();
    assert!(tab.contains("± COMMIT_EDITMSG ●"), "{tab}");
    app.config.icons = Icons::Nerd;
    let text = screen(&app, 120, 30);
    assert!(text.contains("\u{e615} practice.toml"), "{text}");
    assert!(text.contains("\u{f07c} code-racer"), "{text}");
    app.config.icons = Icons::None;
    let text = screen(&app, 120, 30);
    assert!(text.contains("     practice.toml"), "{text}");
}
