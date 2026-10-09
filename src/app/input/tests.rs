use super::*;

fn press(input: &mut TextInput, code: KeyCode) -> Edit {
    input.handle_key(KeyEvent::from(code))
}

fn type_text(input: &mut TextInput, text: &str) {
    for ch in text.chars() {
        press(input, KeyCode::Char(ch));
    }
}

#[test]
fn edits_at_the_cursor() {
    let mut input = TextInput::new("", 24);
    type_text(&mut input, "jen");
    press(&mut input, KeyCode::Left);
    type_text(&mut input, "a");
    assert_eq!(input.value(), "jean");
    assert_eq!(input.before_cursor(), "jea");
    press(&mut input, KeyCode::Delete);
    press(&mut input, KeyCode::Home);
    press(&mut input, KeyCode::Delete);
    assert_eq!(input.value(), "ea");
}

#[test]
fn respects_the_length_limit_in_graphemes() {
    let mut input = TextInput::new("", 3);
    type_text(&mut input, "éé👩‍💻x");
    assert_eq!(input.value(), "éé👩‍💻");
}

#[test]
fn backspace_removes_a_whole_grapheme() {
    let mut input = TextInput::new("a👩‍💻", 10);
    press(&mut input, KeyCode::Backspace);
    assert_eq!(input.value(), "a");
}

#[test]
fn control_w_deletes_the_previous_word() {
    let mut input = TextInput::new("join FK72AD ", 40);
    input.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert_eq!(input.value(), "join ");
}

#[test]
fn deleting_a_word_after_wide_whitespace_keeps_the_whitespace() {
    for space in ['\u{a0}', '\u{3000}'] {
        let text = format!("join{space}FK");
        for key in [
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT),
        ] {
            let mut input = TextInput::new(&text, 40);
            input.handle_key(key);
            assert_eq!(input.value(), format!("join{space}"), "{key:?}");
        }
    }
}

#[test]
fn altgr_characters_are_typed() {
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    let mut input = TextInput::new("", 40);
    for ch in ['@', '\\', '[', 'w', 'a'] {
        assert_eq!(
            input.handle_key(KeyEvent::new(KeyCode::Char(ch), altgr)),
            Edit::Changed
        );
    }
    assert_eq!(input.value(), "@\\[wa");
}

#[test]
fn control_letters_are_shortcuts_not_text() {
    let mut input = TextInput::new("ab", 40);
    input.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    assert_eq!(input.before_cursor(), "", "Ctrl+A goes home");
    assert_eq!(
        input.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
        Edit::Ignored
    );
    input.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    assert_eq!(input.before_cursor(), "ab", "Ctrl+E goes to the end");
    assert_eq!(input.value(), "ab");
}

#[test]
fn control_h_is_a_backspace() {
    let key = normalized(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
    assert_eq!(key.code, KeyCode::Backspace);
    assert!(!erases_word(key));
    let mut input = TextInput::new("join FK", 40);
    input.handle_key(key);
    assert_eq!(input.value(), "join F");
    let plain = KeyEvent::from(KeyCode::Char('h'));
    assert_eq!(normalized(plain), plain);
}

#[test]
fn enter_and_escape_end_editing() {
    let mut input = TextInput::new("x", 5);
    assert_eq!(press(&mut input, KeyCode::Enter), Edit::Submitted);
    assert_eq!(press(&mut input, KeyCode::Esc), Edit::Cancelled);
    assert_eq!(press(&mut input, KeyCode::F(5)), Edit::Ignored);
}

#[test]
fn pasted_text_drops_control_characters() {
    let mut input = TextInput::new("", 20);
    input.insert_str("ab\ncd\t");
    assert_eq!(input.value(), "abcd");
}
