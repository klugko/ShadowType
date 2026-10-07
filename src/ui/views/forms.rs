//! `practice.toml`, `race.toml` and `config.toml`: settings shown as TOML.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Modifier,
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use super::doc;
use crate::{
    app::{
        App, Focus, TextField,
        form::{self, Value},
        input::TextInput,
        practice, race, settings,
    },
    ui::{
        editor::{self, Row},
        format,
        theme::Palette,
    },
};

/// Widest value the hints of a form line up after.
const MAX_VALUE_WIDTH: usize = 28;

/// A line of a form document.
enum Item<'a> {
    Plain(Row),
    Field {
        row: form::Row,
        selected: bool,
        /// What is being typed in the line, while it is.
        editing: Option<&'a TextInput>,
    },
}

impl<'a> Item<'a> {
    /// A form line of `app`, being typed when it takes `text` and that text
    /// is being edited.
    fn field(app: &'a App, row: form::Row, text: Option<TextField>, selected: bool) -> Self {
        Self::Field {
            row,
            selected: selected && app.focus == Focus::Editor,
            editing: text.and_then(|field| app.input_of(field)),
        }
    }
}

pub fn practice(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    let settings = &app.config.practice;
    let fields = practice::fields(settings);
    let selected = app.practice_cursor.index(fields.len());
    let mut items = vec![
        Item::Plain(doc::title("practice.toml", palette)),
        Item::Plain(doc::comment(
            "solo session settings, s starts one from anywhere",
            palette,
        )),
        Item::Plain(doc::blank()),
    ];
    items.extend(fields.iter().enumerate().map(|(index, field)| {
        Item::field(
            app,
            practice::row(settings, *field),
            None,
            index == selected,
        )
    }));
    if let Some(last) = app.history.records().last() {
        items.push(Item::Plain(doc::blank()));
        items.push(Item::Plain(doc::comment(
            format!(
                "last session: {} · {} · {:.0} wpm · {:.0}% accuracy",
                last.mode, last.language, last.wpm, last.accuracy
            ),
            palette,
        )));
    }
    draw(frame, area, items, palette)
}

pub fn race(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    let fields = race::fields(&app.config.race);
    let selected = app.race_cursor.index(fields.len());
    let mut items = vec![
        Item::Plain(doc::title("race.toml", palette)),
        Item::Plain(doc::comment(
            "race your team: everyone types the same text at the same time",
            palette,
        )),
    ];
    for (index, field) in fields.iter().enumerate() {
        if let Some(section) = race::section(*field) {
            items.push(Item::Plain(doc::blank()));
            items.push(Item::Plain(Row::new(vec![Span::styled(
                format!("[{section}]"),
                palette.fg(palette.keyword),
            )])));
        }
        let row = race::row(&app.config, &app.room_code, *field);
        items.push(Item::field(app, row, field.text_field(), index == selected));
    }
    draw(frame, area, items, palette)
}

pub fn settings(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    let selected = app.settings_cursor.index(settings::FIELDS.len());
    let mut items = vec![
        Item::Plain(doc::title("config.toml", palette)),
        Item::Plain(doc::comment("Enter edits or changes a value", palette)),
        Item::Plain(doc::blank()),
    ];
    for (index, field) in settings::FIELDS.iter().enumerate() {
        let row = settings::row(&app.config, *field);
        items.push(Item::field(app, row, field.text_field(), index == selected));
    }
    if let Some(path) = app.config_path() {
        items.push(Item::Plain(doc::blank()));
        items.push(Item::Plain(doc::comment(
            format!("saved in {}", path.display()),
            palette,
        )));
    }
    draw(frame, area, items, palette)
}

/// Draws the form lines; returns where the terminal cursor goes when a
/// value is being typed and shows.
fn draw(
    frame: &mut Frame,
    area: Rect,
    items: Vec<Item<'_>>,
    palette: &Palette,
) -> Option<Position> {
    let columns = Columns::of(&items, area.width, palette);
    let mut cursor = None;
    let mut selected_row = 0;
    let mut rows: Vec<Row> = Vec::with_capacity(items.len());
    for item in items {
        let row = match item {
            Item::Plain(row) => row,
            Item::Field {
                row,
                selected,
                editing,
            } => {
                if selected {
                    selected_row = rows.len();
                }
                let (row, column) = field_row(&row, columns, selected, editing, palette);
                cursor = column.map(|column| (rows.len(), column)).or(cursor);
                row
            }
        };
        rows.push(row);
    }
    editor::number_rows(&mut rows);
    let scroll = editor::scroll_for(selected_row, area.height, rows.len());
    let text = editor::render(frame, area, &rows, scroll, palette);
    let (row, column) = cursor?;
    let row = u16::try_from(row.checked_sub(scroll)?).ok()?;
    let column = u16::try_from(column).unwrap_or(u16::MAX);
    (row < text.height).then_some(Position {
        x: text.x + column.min(text.width.saturating_sub(1)),
        y: text.y + row,
    })
}

/// The columns of a form.
#[derive(Debug, Clone, Copy)]
struct Columns {
    /// Width the keys are padded to.
    key: usize,
    /// Width the values are padded to, for the hints to line up after them.
    value: usize,
    /// Width of the text of a line, next to the line numbers.
    text: usize,
}

impl Columns {
    /// The columns of the form of `items`, in an area `area_width` wide.
    fn of(items: &[Item<'_>], area_width: u16, palette: &Palette) -> Self {
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

/// The row of a form line and, while its value is being typed, the column
/// of the typing cursor in it. The hint follows only when it fits whole.
fn field_row(
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

/// `text` as a comment after `spans`, if any and if it fits whole in
/// `width` columns: a cut example could read as another valid value.
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

/// The line of a value being typed, scrolled to keep its cursor in view
/// with both quotes, and the column of the cursor.
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

/// The comment after a form line: how to end the typing, the key of the
/// selected action, or what the line sets.
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
