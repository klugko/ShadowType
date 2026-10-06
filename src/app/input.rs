//! Single-line text field with an editing cursor.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

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

    /// Text before the cursor, used to place the terminal cursor.
    pub fn before_cursor(&self) -> &str {
        &self.value[..self.cursor]
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Edit {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => Edit::Submitted,
            KeyCode::Esc => Edit::Cancelled,
            KeyCode::Char('w') if control => self.changed(Self::delete_word),
            KeyCode::Char('u') if control => self.changed(Self::clear_before_cursor),
            KeyCode::Char('a') if control => self.changed(Self::home),
            KeyCode::Char('e') if control => self.changed(Self::end),
            KeyCode::Char(ch) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.changed(|input| input.insert(ch))
            }
            KeyCode::Backspace if control => self.changed(Self::delete_word),
            KeyCode::Backspace => self.changed(Self::backspace),
            KeyCode::Delete => self.changed(Self::delete),
            KeyCode::Left => self.changed(Self::left),
            KeyCode::Right => self.changed(Self::right),
            KeyCode::Home => self.changed(Self::home),
            KeyCode::End => self.changed(Self::end),
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
        let before = self.before_cursor();
        let trimmed = before.trim_end();
        let start = trimmed
            .rfind(char::is_whitespace)
            .map_or(0, |index| index + 1);
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
