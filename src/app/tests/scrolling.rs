use code_racer_engine::Stats;

use super::*;
use crate::history::Record;

#[test]
fn scrolling_stops_with_the_last_line_at_the_bottom() {
    let mut app = app();
    app.resize(80, 20);
    let rows = app.viewport.editor_rows();
    assert_eq!(rows, 17);
    command(&mut app, "help");
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.help_scroll, help::LINES.len() - rows);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help_scroll, help::LINES.len() - rows);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.help_scroll, help::LINES.len() - rows - 1);
    app.resize(200, 100);
    assert_eq!(app.help_scroll, 0, "everything fits");
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn history_scrolls_to_its_oldest_session() {
    let mut app = app();
    let stats = Stats::default();
    for _ in 0..3 {
        let record = Record::from_stats("quote".to_owned(), "english".to_owned(), &stats);
        app.history.add(record).expect("in memory");
    }
    app.resize(80, 20);
    command(&mut app, "history");
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(
        app.history_scroll,
        history_log::lines(3).len() - app.viewport.editor_rows()
    );
}
