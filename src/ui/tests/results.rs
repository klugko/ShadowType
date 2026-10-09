use super::*;

#[test]
fn solo_results_show_the_metrics() {
    let mut app = app();
    app.history
        .add(Record {
            mode: "quote".to_owned(),
            ..record(10.0)
        })
        .expect("in memory");
    command(&mut app, "quote");
    let length = app.session_view().expect("a quote").session.target().len();
    let end = type_whole_text(&mut app, Duration::from_secs(20));

    let text = screen_at(&app, 100, 30, end + Duration::from_secs(30));
    let wpm = code_racer_engine::words_per_minute(length, Duration::from_secs(20));
    for expected in [
        "session complete".to_owned(),
        format!("wpm         = {wpm:.1}"),
        "accuracy    = 100.0%".to_owned(),
        "errors      = 0".to_owned(),
        "time        = 0:20.0".to_owned(),
        "new personal best, previous 10.0 wpm".to_owned(),
        "## wpm over time".to_owned(),
    ] {
        assert!(text.contains(&expected), "missing {expected}:\n{text}");
    }
    assert_eq!(chart_end(&text), Some(97), "the chart spans the text");
    let status = status_line(&text);
    assert!(status.contains(&format!(" {wpm:.0} wpm ")), "{status}");
    assert!(
        status.contains(" 00:20 ") && status.ends_with(" 100% "),
        "{status}"
    );
}

#[test]
fn the_results_name_the_characters_missed() {
    let mut app = app();
    command(&mut app, "words 10");
    mistake(&mut app);
    press(&mut app, KeyCode::Backspace);
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let text = screen_at(&app, 100, 30, end + Duration::from_secs(5));
    assert!(text.contains("missed      = { "), "{text}");
}

#[test]
fn the_results_count_up_when_they_come_on_screen() {
    let mut app = app();
    command(&mut app, "words 10");
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let early = screen_at(&app, 100, 30, end);
    assert!(early.contains("wpm         = 0.0"), "{early}");
    app.config.animations = false;
    let still = screen_at(&app, 100, 30, end);
    assert!(!still.contains("wpm         = 0.0"), "{still}");
}
