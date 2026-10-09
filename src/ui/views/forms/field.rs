use ratatui::{style::Modifier, text::Span};
use unicode_width::UnicodeWidthStr;

use super::{Item, doc};
use crate::{
    app::{
        form::{self, Value},
        input::TextInput,
    },
    ui::{
        editor::{self, Row},
        format,
        theme::Palette,
    },
};

/// Widest value the hints of a form line up after.
const MAX_VALUE_WIDTH: usize = 28;

#[derive(Debug, Clone, Copy)]
pub(super) struct Columns {
    /// Width the keys are padded to.
    key: usize,
    /// Width the values are padded to, for the hints to line up after them.
    value: usize,
    /// Width of the text of a line, next to the line numbers.
    text: usize,
}

impl Columns {
    pub(super) fn of(items: &[Item<'_>], area_width: u16, palette: &Palette) -> Self {
        let rows = items.iter().filter_map(|item| match item {
            Item::Field { row, .. } if !matches!(row.value, Value::Action(_)) => Some(row),
            _ => None,
        });
        let (key, value) = rows.fold((0, 0), |(keys, values), row| {
            let value = value_span(&row.value, palette).width();
            (keys.max(row.key.width()), values.max(value))
        });
        Self {
            key,
            value: value.min(MAX_VALUE_WIDTH),
            text: usize::from(editor::text_width(area_width, items.len())),
        }
    }
}

/**
 * The row of a form line and, while its value is being typed, the column
 * of the typing cursor in it. The hint follows only when it fits whole.
 */
pub(super) fn field_row(
    row: &form::Row,
    columns: Columns,
    selected: bool,
    editing: Option<&TextInput>,
    palette: &Palette,
) -> (Row, Option<usize>) {
    let (mut spans, cursor) = match (&row.value, editing) {
        (Value::Action(label), _) => (vec![action(label, palette)], None),
        (value, None) => (
            setting(row.key, value_span(value, palette), columns, palette),
            None,
        ),
        (_, Some(input)) => {
            let (spans, cursor) = typed_setting(row.key, input, columns, palette);
            (spans, Some(cursor))
        }
    };
    let hint = hint(row, selected, editing.is_some());
    spans.extend(comment_after(&spans, hint, columns.text, palette));
    (Row::new(spans).current(selected), cursor)
}

/**
 * `text` as a comment after `spans`, if any and if it fits whole in
 * `width` columns: a cut example could read as another valid value.
 */
fn comment_after(
    spans: &[Span<'_>],
    text: &str,
    width: usize,
    palette: &Palette,
) -> Option<Span<'static>> {
    let comment = format!("  # {text}");
    let used: usize = spans.iter().map(Span::width).sum();
    (!text.is_empty() && used + comment.width() <= width)
        .then(|| Span::styled(comment, palette.fg(palette.comment)))
}

fn action(label: &str, palette: &Palette) -> Span<'static> {
    Span::styled(
        format!("▶ {label}"),
        palette.fg(palette.accent).add_modifier(Modifier::BOLD),
    )
}

/// `key = value`, padded for the hints of the form to line up after it.
fn setting(
    key: &str,
    value: Span<'static>,
    columns: Columns,
    palette: &Palette,
) -> Vec<Span<'static>> {
    let padding = columns.value.saturating_sub(value.width());
    let mut spans = doc::assignment(key, columns.key, value, palette);
    spans.push(Span::raw(" ".repeat(padding)));
    spans
}

/**
 * The line of a value being typed, scrolled to keep its cursor in view
 * with both quotes, and the column of the cursor.
 */
fn typed_setting(
    key: &str,
    input: &TextInput,
    columns: Columns,
    palette: &Palette,
) -> (Vec<Span<'static>>, usize) {
    let start = doc::value_column(key, columns.key);
    let quotes = 2 * doc::QUOTE.width();
    let view = format::input_view(input, columns.text.saturating_sub(start + quotes));
    let cursor = start + doc::QUOTE.width() + view.cursor;
    let value = doc::string(&view.text, palette);
    (setting(key, value, columns, palette), cursor)
}

/**
 * The comment after a form line: how to end the typing, the key of the
 * selected action, or what the line sets.
 */
fn hint(row: &form::Row, selected: bool, editing: bool) -> &str {
    match (&row.value, editing) {
        (_, true) => "Enter saves, Esc cancels",
        (Value::Action(_), false) if selected => "Enter",
        _ => &row.hint,
    }
}

fn value_span(value: &Value, palette: &Palette) -> Span<'static> {
    match value {
        Value::Text(text) => doc::string(text, palette),
        Value::Number(number) => Span::styled(number.to_string(), palette.fg(palette.number)),
        Value::Bool(flag) => Span::styled(flag.to_string(), palette.fg(palette.keyword)),
        Value::Action(label) => Span::raw(*label),
    }
}
