use super::*;

fn hits_of(app: &App, width: u16, height: u16, now: Instant) -> crate::app::mouse::Hits {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    let mut hits = crate::app::mouse::Hits::default();
    terminal
        .draw(|frame| {
            hits = draw(frame, app, now);
        })
        .expect("draw");
    hits
}

#[test]
fn what_the_screen_shows_is_where_a_click_lands() {
    use crate::app::{Buffer, mouse::Target};
    let mut app = app();
    add_records(&mut app, &[70.0]);
    command(&mut app, "words 10");
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let now = end + Duration::from_secs(5);
    app.handle_key(KeyEvent::from(KeyCode::Char('e')), now);
    assert_eq!(
        app.buffer,
        Buffer::Practice,
        "the results open the settings"
    );
    let terminal = drawn(&app, 120, 30, now);
    let hits = hits_of(&app, 120, 30, now);
    let (x, y) = find(&terminal, "history.log").expect("the entry");
    assert_eq!(hits.at(x, y), Some(Target::Entry(Buffer::History)));
    let (x, y) = find(&terminal, "notes.md").expect("the session tab");
    assert_eq!(hits.at(x, y), Some(Target::Tab(Buffer::Session)));
    let (_, folder) = find(&terminal, "▾ session").expect("the session folder");
    assert_eq!(hits.at(5, folder + 1), Some(Target::Entry(Buffer::Session)));
    let (x, y) = find(&terminal, "language    =").expect("a form line");
    assert_eq!(hits.at(x, y), Some(Target::FormLine(1)));
    let (x, y) = find(&terminal, " NORMAL ").expect("the mode");
    assert_eq!(hits.at(x, y), Some(Target::Mode));
    let (x, y) = find(&terminal, "~").expect("the end of the buffer");
    assert_eq!(hits.at(x, y), Some(Target::Editor));
}

#[test]
fn the_keys_under_the_results_can_be_clicked() {
    use crate::app::mouse::Target;
    let mut app = app();
    command(&mut app, "words 10");
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let later = end + Duration::from_secs(5);
    let terminal = drawn(&app, 100, 30, later);
    let hits = hits_of(&app, 100, 30, later);
    let (x, y) = find(&terminal, "r  new text").expect("the keys");
    assert_eq!(
        hits.at(x + 3, y),
        Some(Target::Key {
            code: KeyCode::Char('r'),
            times: 1
        })
    );
}
