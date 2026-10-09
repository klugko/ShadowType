use super::*;

#[test]
fn history_shows_records_and_a_chart() {
    let mut app = app();
    for wpm in [60.0, 64.0, 70.0, 68.0, 75.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    command(&mut app, "history");
    let text = screen(&app, 120, 40);
    for expected in ["5 sessions", "best", "75", "words 50", "┤"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    assert_eq!(chart_end(&text), Some(117), "two columns short of the edge");
}

#[test]
fn history_draws_every_line_of_its_layout_and_no_other() {
    use crate::app::history_log::{CHART_SESSIONS, lines};
    for sessions in [0, 1, 2, CHART_SESSIONS + 1] {
        let mut app = app();
        for _ in 0..sessions {
            app.history.add(record(70.0)).expect("in memory");
        }
        command(&mut app, "history");
        let count = lines(sessions).len();
        let height = u16::try_from(count + 4)
            .expect("a small screen")
            .max(MIN_HEIGHT);
        let text = screen(&app, 120, height);
        let numbered = |number: usize| {
            text.lines().any(|line| {
                line.split('│')
                    .nth(1)
                    .is_some_and(|editor| editor.trim_start().starts_with(&format!("{number}  ")))
            })
        };
        assert!(
            numbered(count),
            "line {count} of {sessions} sessions:\n{text}"
        );
        assert!(!numbered(count + 1), "{sessions} sessions:\n{text}");
    }
}

#[test]
fn the_oldest_session_can_be_scrolled_into_view() {
    let mut app = app();
    app.history
        .add(Record {
            mode: "words 10".to_owned(),
            ..record(50.0)
        })
        .expect("in memory");
    for wpm in [60.0, 70.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    app.resize(80, 20);
    command(&mut app, "history");
    press(&mut app, KeyCode::Char('G'));
    let text = screen(&app, 80, 20);
    let last_editor_row = text.lines().nth(17).unwrap_or_default();
    assert!(last_editor_row.contains("words 10"), "{text}");
}
