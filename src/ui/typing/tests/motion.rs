use super::*;
use crate::{
    app::{
        ink::{DRYING_TIME, Ink},
        text_event::TextEvent,
    },
    ui::typing::cursor::{BREATH, BREATHE_AFTER},
};

#[test]
fn fresh_ink_glows_then_dries() {
    let mut session = TypingSession::new("abc", SessionOptions::default());
    let mut ink = Ink::default();
    let now = Instant::now();
    ink.apply(TextEvent::Typed('a'), &mut session, now);
    let palette = Palette::of(Theme::Editor);
    let view = SessionView {
        ink: Some(&ink),
        ..view(&session, None)
    };
    let glowing = Moment {
        trail: false,
        ..Moment::moving(now)
    };
    let fresh = layout(&view, 20, &palette, false, glowing);
    let trailing = layout(&view, 20, &palette, false, Moment::moving(now));
    assert_eq!(
        style_at(&trailing, 0, 0).fg,
        Some(palette.strong),
        "the trail shows fresh text instead of the glow"
    );
    let dry = layout(
        &view,
        20,
        &palette,
        false,
        Moment::moving(now + DRYING_TIME),
    );
    let still = layout(&view, 20, &palette, false, Moment::still(now));
    assert_eq!(style_at(&fresh, 0, 0).fg, palette.glow);
    assert_eq!(style_at(&dry, 0, 0).fg, Some(palette.strong));
    assert_eq!(
        style_at(&still, 0, 0).fg,
        Some(palette.strong),
        "nothing glows without animations"
    );
    assert!(is_moving(&view, Moment::moving(now)));
    assert!(!is_moving(&view, Moment::still(now)));
}

#[test]
fn the_cursor_leaves_a_fading_trail_of_its_colour() {
    let mut session = TypingSession::new("abcdef", SessionOptions::default());
    let mut ink = Ink::default();
    let now = Instant::now();
    for ch in "abc".chars() {
        ink.apply(TextEvent::Typed(ch), &mut session, now);
    }
    let palette = Palette::of(Theme::Editor);
    let view = SessionView {
        ink: Some(&ink),
        ..view(&session, None)
    };
    let trailing = layout(&view, 20, &palette, true, Moment::moving(now));
    let behind = style_at(&trailing, 0, 2).bg;
    assert!(
        behind.is_some() && behind != palette.cursorline.bg,
        "{behind:?}"
    );
    assert_eq!(
        style_at(&trailing, 0, 3),
        palette.cursor,
        "the cursor itself"
    );
    let without = Moment {
        trail: false,
        ..Moment::moving(now)
    };
    let plain = layout(&view, 20, &palette, true, without);
    assert_eq!(style_at(&plain, 0, 2).bg, None, "no trail when it is off");
    let mono = layout(
        &view,
        20,
        &Palette::of(Theme::Mono),
        true,
        Moment::moving(now),
    );
    assert_eq!(
        style_at(&mono, 0, 2).bg,
        None,
        "nor where colours cannot fade"
    );
}

#[test]
fn the_cursor_breathes_only_once_the_player_pauses() {
    let mut session = TypingSession::new("abc", SessionOptions::default());
    let mut ink = Ink::default();
    let now = Instant::now();
    ink.apply(TextEvent::Typed('a'), &mut session, now);
    let palette = Palette::of(Theme::Editor);
    let view = SessionView {
        ink: Some(&ink),
        ..view(&session, None)
    };
    let typing = cursor_style(&view, &palette, Moment::moving(now));
    assert_eq!(typing, palette.cursor);
    let half_breath = BREATHE_AFTER + BREATH / 2;
    let pausing = cursor_style(&view, &palette, Moment::moving(now + half_breath));
    assert_ne!(pausing.bg, palette.cursor.bg);
    let mono = Palette::of(Theme::Mono);
    assert_eq!(
        cursor_style(&view, &mono, Moment::moving(now + half_breath)),
        mono.cursor
    );
}
