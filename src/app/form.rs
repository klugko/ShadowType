//! Settings buffers presented as editable TOML-like lines.

/// One line of a form, rendered as `key = value  # hint`.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: &'static str,
    pub value: Value,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Number(u32),
    Bool(bool),
    /// A command run with Enter, shown as `▶ label`.
    Action(&'static str),
}

/// Direction of a value change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Previous,
}

impl Row {
    pub fn new(key: &'static str, value: Value) -> Self {
        Self {
            key,
            value,
            hint: String::new(),
        }
    }

    pub fn action(label: &'static str) -> Self {
        Self::new("", Value::Action(label))
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }
}

/// Hint listing every choice, such as `words · time · quote · code`.
pub fn choices<T: std::fmt::Display>(values: impl IntoIterator<Item = T>) -> String {
    values
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Moves to the next or previous element of `values`, wrapping around.
pub fn cycle<T: Copy + PartialEq>(values: &[T], current: T, step: Step) -> T {
    let Some(index) = values.iter().position(|value| *value == current) else {
        return values.first().copied().unwrap_or(current);
    };
    let len = values.len();
    let next = match step {
        Step::Next => (index + 1) % len,
        Step::Previous => (index + len - 1) % len,
    };
    values[next]
}

/// Moves through presets, starting from the closest one when `current` is custom.
pub fn cycle_preset(presets: &[u16], current: u16, step: Step) -> u16 {
    match step {
        Step::Next => presets
            .iter()
            .copied()
            .find(|preset| *preset > current)
            .or_else(|| presets.first().copied()),
        Step::Previous => presets
            .iter()
            .rev()
            .copied()
            .find(|preset| *preset < current)
            .or_else(|| presets.last().copied()),
    }
    .unwrap_or(current)
}

/// Selected line of a form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    index: usize,
}

impl Cursor {
    pub fn index(self, len: usize) -> usize {
        self.index.min(len.saturating_sub(1))
    }

    pub fn down(&mut self, len: usize) {
        self.index = (self.index(len) + 1).min(len.saturating_sub(1));
    }

    pub fn up(&mut self, len: usize) {
        self.index = self.index(len).saturating_sub(1);
    }

    pub fn last(&mut self, len: usize) {
        self.index = len.saturating_sub(1);
    }

    pub fn first(&mut self) {
        self.index = 0;
    }

    /// Selects the line `index` of a form of `len` lines.
    pub fn select(&mut self, index: usize, len: usize) {
        self.index = index.min(len.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_wraps_in_both_directions() {
        let values = [1, 2, 3];
        assert_eq!(cycle(&values, 3, Step::Next), 1);
        assert_eq!(cycle(&values, 1, Step::Previous), 3);
        assert_eq!(cycle(&values, 9, Step::Next), 1);
    }

    #[test]
    fn presets_snap_from_custom_values() {
        let presets = [10, 25, 50, 100];
        assert_eq!(cycle_preset(&presets, 30, Step::Next), 50);
        assert_eq!(cycle_preset(&presets, 30, Step::Previous), 25);
        assert_eq!(cycle_preset(&presets, 100, Step::Next), 10);
        assert_eq!(cycle_preset(&presets, 10, Step::Previous), 100);
    }

    #[test]
    fn cursor_stays_within_the_form() {
        let mut cursor = Cursor::default();
        cursor.up(4);
        assert_eq!(cursor.index(4), 0);
        for _ in 0..10 {
            cursor.down(4);
        }
        assert_eq!(cursor.index(4), 3);
        assert_eq!(cursor.index(2), 1, "shrinking forms clamp the cursor");
    }

    #[test]
    fn choices_are_joined_with_dots() {
        assert_eq!(choices(["a", "b"]), "a · b");
    }
}
