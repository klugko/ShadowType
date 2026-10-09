use crossterm::event::KeyModifiers;

use super::*;
use crate::{
    app::{Overrides, practice, test_support::type_next},
    cli::Launch,
    config::{Config, Mode},
    history::History,
};

fn app() -> App {
    let config = Config {
        username: "ada".to_owned(),
        ..Config::default()
    };
    App::new(
        config,
        &Overrides::default(),
        None,
        History::in_memory(),
        Vec::new(),
        Launch::Home,
    )
}

fn click(app: &mut App, hits: &Hits, (column, row): (u16, u16)) -> bool {
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(event, hits, Instant::now())
}

fn hits(targets: &[Target]) -> Hits {
    let mut hits = Hits::default();
    for (row, target) in (0..).zip(targets) {
        hits.add(Rect::new(0, row, 10, 1), *target);
    }
    hits
}

#[test]
fn the_topmost_region_wins() {
    let mut hits = Hits::default();
    hits.add(Rect::new(0, 0, 80, 20), Target::Editor);
    hits.add(Rect::new(2, 3, 5, 1), Target::Mode);
    assert_eq!(hits.at(3, 3), Some(Target::Mode));
    assert_eq!(hits.at(30, 3), Some(Target::Editor));
    assert_eq!(hits.at(90, 3), None);
}

#[test]
fn clicking_an_explorer_entry_opens_it() {
    let mut app = app();
    let hits = hits(&[Target::Entry(Buffer::History)]);
    assert!(click(&mut app, &hits, (1, 0)));
    assert_eq!((app.buffer, app.focus), (Buffer::History, Focus::Editor));
}

#[test]
fn a_form_line_is_selected_then_changed() {
    let mut app = app();
    app.buffer = Buffer::Practice;
    let words = 2;
    let hits = hits(&[Target::FormLine(words)]);
    click(&mut app, &hits, (1, 0));
    assert_eq!(app.practice_cursor.index(6), words);
    assert_eq!(app.config.practice.word_count, 50, "selected first");
    click(&mut app, &hits, (1, 0));
    assert_eq!(app.config.practice.word_count, 100, "then changed");
    assert_eq!(app.config.practice.mode, Mode::Words);
}

#[test]
fn an_action_line_acts_at_once() {
    let mut app = app();
    app.buffer = Buffer::Practice;
    let start = practice::fields(&app.config.practice).len() - 1;
    let hits = hits(&[Target::FormLine(start)]);
    click(&mut app, &hits, (1, 0));
    assert!(app.solo().is_some(), "the session started");
    assert_eq!(app.buffer, Buffer::Session);
}

#[test]
fn clicks_never_leave_the_text_being_typed() {
    let mut app = app();
    app.start_practice();
    type_next(&mut app, Instant::now());
    let hits = hits(&[Target::Entry(Buffer::Help)]);
    click(&mut app, &hits, (1, 0));
    assert!(app.is_typing());
}

#[test]
fn moving_the_mouse_changes_nothing() {
    let mut app = app();
    let moved = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 1,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    assert!(!app.handle_mouse(moved, &Hits::default(), Instant::now()));
}

#[test]
fn the_mouse_can_be_left_to_the_terminal() {
    let mut app = app();
    app.config.mouse = false;
    let hits = hits(&[Target::Entry(Buffer::History)]);
    assert!(!click(&mut app, &hits, (1, 0)));
    assert_eq!(app.buffer, Buffer::Practice);
}

#[test]
fn the_wheel_scrolls_the_document() {
    let mut app = app();
    app.resize(80, 20);
    app.buffer = Buffer::Help;
    app.focus = Focus::Editor;
    let hits = hits(&[Target::Editor]);
    let wheel = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(wheel, &hits, Instant::now());
    assert_eq!(app.help_scroll, WHEEL_LINES);
}

#[test]
fn a_shown_key_is_pressed_by_a_click() {
    let mut app = app();
    let hits = hits(&[Target::Key {
        code: KeyCode::Char('?'),
        times: 1,
    }]);
    click(&mut app, &hits, (1, 0));
    assert_eq!(app.buffer, Buffer::Help);
}

#[test]
fn clicking_a_palette_entry_runs_it() {
    let mut app = app();
    app.open_palette();
    let index = app
        .palette_matches()
        .iter()
        .position(|found| found.entry.title == "Open help.md")
        .expect("the entry");
    let hits = hits(&[Target::PaletteEntry(index)]);
    click(&mut app, &hits, (1, 0));
    assert!(app.palette.is_none());
    assert_eq!(app.buffer, Buffer::Help);
}
