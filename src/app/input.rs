//! Keyboard input: single-line text fields, and the rules that tell typed
//! characters from shortcuts everywhere in the application.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

/// The character a key types, if any.
///
/// Plain keys type, and so do keys held with both Ctrl and Alt: crossterm
/// reports AltGr that way on Windows, so `@` on a French keyboard arrives as
/// Ctrl+Alt+`@`. Ctrl or Alt alone makes a shortcut.
pub fn typed_char(key: KeyEvent) -> Option<char> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char(ch) if control == alt => Some(ch),
        _ => None,
    }
}

/// The lowercase letter of a Ctrl shortcut such as Ctrl+W, `None` for any
/// other key, AltGr characters included.
pub fn control_letter(key: KeyEvent) -> Option<char> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char(ch) if control && !alt => Some(ch.to_ascii_lowercase()),
        _ => None,
    }
}

/// Whether a key erases the previous word: Alt+Backspace, or Ctrl+Backspace
/// on the terminals that tell it from Backspace.
pub fn erases_word(key: KeyEvent) -> bool {
    key.code == KeyCode::Backspace
        && key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Ctrl+H is the control character of Backspace: some terminals send it for
/// Backspace, most for Ctrl+Backspace. Either way it erases one character.
pub fn normalized(key: KeyEvent) -> KeyEvent {
    if control_letter(key) == Some('h') {
        KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            ..key
        }
    } else {
        key
    }
}

/// What a key press did to a [`TextInput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    Submitted,
    Cancelled,
    Changed,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInput {
    value: String,
    cursor: usize,
    max_graphemes: usize,
}

impl TextInput {
    pub fn new(value: &str, max_graphemes: usize) -> Self {
        let value: String = value.graphemes(true).take(max_graphemes).collect();
        Self {
            cursor: value.len(),
            value,
            max_graphemes,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn before_cursor(&self) -> &str {
        &self.value[..self.cursor]
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Edit {
        if let Some(ch) = typed_char(key) {
            return self.changed(|input| input.insert(ch));
        }
        if let Some(letter) = control_letter(key) {
            return self.shortcut(letter);
        }
        match key.code {
            KeyCode::Enter => Edit::Submitted,
            KeyCode::Esc => Edit::Cancelled,
            KeyCode::Backspace if erases_word(key) => self.changed(Self::delete_word),
            KeyCode::Backspace => self.changed(Self::backspace),
            KeyCode::Delete => self.changed(Self::delete),
            KeyCode::Left => self.changed(Self::left),
            KeyCode::Right => self.changed(Self::right),
            KeyCode::Home => self.changed(Self::home),
            KeyCode::End => self.changed(Self::end),
            _ => Edit::Ignored,
        }
    }

    /// Readline shortcuts, as in a shell.
    fn shortcut(&mut self, letter: char) -> Edit {
        match letter {
            'w' => self.changed(Self::delete_word),
            'u' => self.changed(Self::clear_before_cursor),
            'a' => self.changed(Self::home),
            'e' => self.changed(Self::end),
            _ => Edit::Ignored,
        }
    }

    pub fn insert_str(&mut self, text: &str) {
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            self.insert(ch);
        }
    }

    fn changed(&mut self, edit: impl FnOnce(&mut Self)) -> Edit {
        edit(self);
        Edit::Changed
    }

    fn insert(&mut self, ch: char) {
        let mut candidate = self.value.clone();
        candidate.insert(self.cursor, ch);
        if candidate.graphemes(true).count() <= self.max_graphemes {
            self.value = candidate;
            self.cursor += ch.len_utf8();
        }
    }

    fn backspace(&mut self) {
        if let Some(start) = self.previous_boundary() {
            self.value.replace_range(start..self.cursor, "");
            self.cursor = start;
        }
    }

    fn delete(&mut self) {
        if let Some(end) = self.next_boundary() {
            self.value.replace_range(self.cursor..end, "");
        }
    }

    fn delete_word(&mut self) {
        let start = self
            .before_cursor()
            .trim_end()
            .trim_end_matches(|ch: char| !ch.is_whitespace())
            .len();
        self.value.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    fn clear_before_cursor(&mut self) {
        self.value.replace_range(..self.cursor, "");
        self.cursor = 0;
    }

    fn left(&mut self) {
        if let Some(start) = self.previous_boundary() {
            self.cursor = start;
        }
    }

    fn right(&mut self) {
        if let Some(end) = self.next_boundary() {
            self.cursor = end;
        }
    }

    fn home(&mut self) {
        self.cursor = 0;
    }

    fn end(&mut self) {
        self.cursor = self.value.len();
    }

    fn previous_boundary(&self) -> Option<usize> {
        self.before_cursor()
            .grapheme_indices(true)
            .next_back()
            .map(|(index, _)| index)
    }

    fn next_boundary(&self) -> Option<usize> {
        self.value[self.cursor..]
            .graphemes(true)
            .next()
            .map(|grapheme| self.cursor + grapheme.len())
    }
}

#[cfg(test)]
mod tests {
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
}
