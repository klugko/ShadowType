use super::*;

#[test]
fn the_mascot_lives_at_the_bottom_of_the_explorer_when_there_is_room() {
    let mut app = app();
    app.resize(120, 30);
    let text = screen(&app, 120, 30);
    let sidebar: Vec<String> = text
        .lines()
        .map(|line| line.chars().take(usize::from(SIDEBAR_WIDTH)).collect())
        .collect();
    let body = sidebar.len() - 2;
    let ghost = sidebar[..body]
        .iter()
        .rposition(|line| line.contains('█'))
        .expect("the ghost");
    assert!(ghost >= body - 3, "at the bottom:\n{text}");
    assert!(chrome::shows_mascot(&app));
    app.config.mascot = false;
    assert!(!screen(&app, 120, 30).contains('█'));
    app.config.mascot = true;
    add_records(&mut app, &[70.0]);
    app.resize(120, MIN_HEIGHT);
    assert!(
        !chrome::shows_mascot(&app),
        "no room under the explorer and the records"
    );
    assert!(!screen(&app, 120, MIN_HEIGHT).contains('█'));
}

#[test]
fn nothing_moves_once_the_mascot_is_asleep_or_animations_are_off() {
    let mut app = app();
    app.resize(120, 30);
    let start = Instant::now();
    assert_eq!(frame_period(&app, start), Some(MASCOT_FRAME));
    let napping = start + crate::app::mascot::NAP_AFTER + Duration::from_secs(1);
    assert_eq!(
        frame_period(&app, napping),
        None,
        "asleep, no processor time"
    );
    assert!(screen_at(&app, 120, 30, napping).contains('Z'), "it snores");
    app.config.animations = false;
    assert_eq!(frame_period(&app, start), None);
}

#[test]
fn typing_redraws_often_while_the_ink_dries() {
    let mut app = app();
    command(&mut app, "words 10");
    let now = Instant::now();
    type_next(&mut app, now);
    assert_eq!(frame_period(&app, now), Some(TEXT_FRAME));
}
