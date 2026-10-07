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
        Item::Plain(doc::blank()),
        Item::Plain(Row::new(doc::assignment(
            "server",
            6,
            doc::string(&app.config.multiplayer.server, palette),
            palette,
        ))),
    ];
    for (index, field) in fields.iter().enumerate() {
        if let Some(section) = race::section(*field) {
            items.push(Item::Plain(doc::blank()));
            items.push(Item::Plain(Row::new(vec![Span::styled(
                format!("[{section}]"),
                palette.fg(palette.keyword),
            )])));
        }
        let row = race::row(&app.config.race, &app.room_code, *field);
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
    let (key_width, value_width) = column_widths(&items, palette);
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
                let (row, column) =
                    field_row(&row, key_width, value_width, selected, editing, palette);
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

/// Widths the keys and the values of the form line up on.
fn column_widths(items: &[Item<'_>], palette: &Palette) -> (usize, usize) {
    let rows = items.iter().filter_map(|item| match item {
        Item::Field { row, .. } if !matches!(row.value, Value::Action(_)) => Some(row),
        _ => None,
    });
    let (keys, values) = rows.fold((0, 0), |(keys, values), row| {
        let value = value_span(&row.value, palette).width();
        (keys.max(row.key.width()), values.max(value))
    });
    (keys, values.min(MAX_VALUE_WIDTH))
}

/// The row of a form line and, while its value is being typed, the column
/// of the typing cursor in it.
fn field_row(
    row: &form::Row,
    key_width: usize,
    value_width: usize,
    selected: bool,
    editing: Option<&TextInput>,
    palette: &Palette,
) -> (Row, Option<usize>) {
    let (mut spans, cursor) = match (&row.value, editing) {
        (Value::Action(label), _) => (
            vec![Span::styled(
                format!("▶ {label}"),
                palette.fg(palette.accent).add_modifier(Modifier::BOLD),
            )],
            None,
        ),
        (value, editing) => {
            let shown = editing.map_or_else(
                || value_span(value, palette),
                |input| doc::string(input.value(), palette),
            );
            let padding = value_width.saturating_sub(shown.width());
            let mut spans = doc::assignment(row.key, key_width, shown, palette);
            let before_value: usize = spans[..spans.len() - 1].iter().map(Span::width).sum();
            let cursor = editing
                .map(|input| before_value + doc::QUOTE.width() + input.before_cursor().width());
            spans.push(Span::raw(" ".repeat(padding)));
            (spans, cursor)
        }
    };
    let hint = match (&row.value, editing) {
        (_, Some(_)) => "Enter saves, Esc cancels".to_owned(),
        (Value::Action(_), None) if selected => "Enter".to_owned(),
        _ => row.hint.clone(),
    };
    if !hint.is_empty() {
        spans.push(Span::styled(
            format!("  # {hint}"),
            palette.fg(palette.comment),
        ));
    }
    (Row::new(spans).current(selected), cursor)
}

fn value_span(value: &Value, palette: &Palette) -> Span<'static> {
    match value {
        Value::Text(text) => doc::string(text, palette),
        Value::Number(number) => Span::styled(number.to_string(), palette.fg(palette.number)),
        Value::Bool(flag) => Span::styled(flag.to_string(), palette.fg(palette.keyword)),
        Value::Action(label) => Span::raw(*label),
    }
}
