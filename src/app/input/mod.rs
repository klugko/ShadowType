/*!
 * Keyboard input: single-line text fields, and the rules that tell typed
 * characters from shortcuts everywhere in the application.
 */

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

/**
 * The character a key types, if any.
 *
 * Plain keys type, and so do keys held with both Ctrl and Alt: crossterm
 * reports AltGr that way on Windows, so `@` on a French keyboard arrives as
 * Ctrl+Alt+`@`. Ctrl or Alt alone makes a shortcut.
 */
pub fn typed_char(key: KeyEvent) -> Option<char> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char(ch) if control == alt => Some(ch),
        _ => None,
    }
}

/**
 * The lowercase letter of a Ctrl shortcut such as Ctrl+W, `None` for any
 * other key, AltGr characters included.
 */
pub fn control_letter(key: KeyEvent) -> Option<char> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char(ch) if control && !alt => Some(ch.to_ascii_lowercase()),
        _ => None,
    }
}

/**
 * Whether a key erases the previous word: Alt+Backspace, or Ctrl+Backspace
 * on the terminals that tell it from Backspace.
 */
pub fn erases_word(key: KeyEvent) -> bool {
    key.code == KeyCode::Backspace
        && key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/**
 * Ctrl+H is the control character of Backspace: some terminals send it for
 * Backspace, most for Ctrl+Backspace. Either way it erases one character.
 */
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
mod tests;
