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
        practice, race, settings,
    },
    ui::{
        editor::{self, Row},
        theme::Palette,
    },
};

/// A line of a form document.
enum Item {
    Plain(Row),
    Field {
        row: form::Row,
        selected: bool,
        editing: Option<TextField>,
    },
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
    items.extend(fields.iter().enumerate().map(|(index, field)| Item::Field {
        row: practice::row(settings, *field),
        selected: index == selected && app.focus == Focus::Editor,
        editing: None,
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
    draw(frame, area, app, items, palette)
}

pub fn race(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    let fields = race::fields(&app.race_settings);
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
        let editing = (*field == race::Field::Room)
            .then_some(TextField::RoomCode)
            .filter(|_| editing_field(app) == Some(TextField::RoomCode));
        items.push(Item::Field {
            row: race::row(&app.race_settings, &app.room_code, *field),
            selected: index == selected && app.focus == Focus::Editor,
            editing,
        });
    }
    draw(frame, area, app, items, palette)
}

pub fn settings(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    let selected = app.settings_cursor.index(settings::FIELDS.len());
    let mut items = vec![
        Item::Plain(doc::title("config.toml", palette)),
        Item::Plain(doc::comment("Enter edits or changes a value", palette)),
        Item::Plain(doc::blank()),
    ];
    for (index, field) in settings::FIELDS.iter().enumerate() {
        let text_field = match field {
            settings::Field::Username => Some(TextField::Username),
            settings::Field::Server => Some(TextField::Server),
            settings::Field::Theme => None,
        };
        items.push(Item::Field {
            row: settings::row(&app.config, *field),
            selected: index == selected && app.focus == Focus::Editor,
            editing: text_field.filter(|field| editing_field(app) == Some(*field)),
        });
    }
    if let Some(path) = app.config_path() {
        items.push(Item::Plain(doc::blank()));
        items.push(Item::Plain(doc::comment(
            format!("saved in {}", path.display()),
            palette,
        )));
    }
    draw(frame, area, app, items, palette)
}

fn editing_field(app: &App) -> Option<TextField> {
    app.editing.as_ref().map(|edit| edit.field)
}

fn draw(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    items: Vec<Item>,
    palette: &Palette,
) -> Option<Position> {
    let key_width = items
        .iter()
        .filter_map(|item| match item {
            Item::Field { row, .. } => Some(row.key.width()),
            Item::Plain(_) => None,
        })
        .max()
        .unwrap_or(0);
    let value_width = items
        .iter()
        .filter_map(|item| match item {
            Item::Field { row, .. } => Some(value_text(&row.value).width()),
            Item::Plain(_) => None,
        })
        .max()
        .unwrap_or(0)
        .min(28);
    let mut cursor = None;
    let mut selected_row = 0;
    let mut rows: Vec<Row> = Vec::with_capacity(items.len());
    for item in items {
        rows.push(match item {
            Item::Plain(row) => row,
            Item::Field {
                row,
                selected,
                editing,
            } => {
                if selected {
                    selected_row = rows.len();
                }
                if editing.is_some() {
                    cursor = edit_cursor(app, rows.len(), key_width);
                }
                field_row(
                    &row,
                    key_width,
                    value_width,
                    selected,
                    editing.is_some(),
                    app,
                    palette,
                )
            }
        });
    }
    editor::number_rows(&mut rows);
    let scroll = editor::scroll_for(selected_row, area.height, rows.len());
    let text = editor::render(frame, area, &rows, scroll, palette);
    let (row, column) = cursor?;
    let row = u16::try_from(row.checked_sub(scroll)?).ok()?;
    (row < text.height).then_some(Position {
        x: text.x + column.min(text.width.saturating_sub(1)),
        y: text.y + row,
    })
}

fn field_row(
    row: &form::Row,
    key_width: usize,
    value_width: usize,
    selected: bool,
    editing: bool,
    app: &App,
    palette: &Palette,
) -> Row {
    let mut spans = match &row.value {
        Value::Action(label) => vec![Span::styled(
            format!("▶ {label}"),
            palette.fg(palette.accent).add_modifier(Modifier::BOLD),
        )],
        value => {
            let shown = match (&app.editing, editing) {
                (Some(edit), true) => doc::string(edit.input.value(), palette),
                _ => value_span(value, palette),
            };
            let padding = value_width.saturating_sub(shown.content.width());
            let mut spans = doc::assignment(row.key, key_width, shown, palette);
            spans.push(Span::raw(" ".repeat(padding)));
            spans
        }
    };
    let hint = if editing {
        "Enter saves, Esc cancels".to_owned()
    } else if matches!(row.value, Value::Action(_)) && selected {
        "Enter".to_owned()
    } else {
        row.hint.clone()
    };
    if !hint.is_empty() {
        spans.push(Span::styled(
            format!("  # {hint}"),
            palette.fg(palette.comment),
        ));
    }
    Row::new(spans).current(selected)
}

fn value_span(value: &Value, palette: &Palette) -> Span<'static> {
    match value {
        Value::Text(text) => doc::string(text, palette),
        Value::Number(number) => Span::styled(number.to_string(), palette.fg(palette.number)),
        Value::Bool(flag) => Span::styled(flag.to_string(), palette.fg(palette.keyword)),
        Value::Action(label) => Span::raw((*label).to_owned()),
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Text(text) => format!("\"{text}\""),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Action(_) => String::new(),
    }
}

/// Column of the terminal cursor inside the edited value.
fn edit_cursor(app: &App, row: usize, key_width: usize) -> Option<(usize, u16)> {
    let edit = app.editing.as_ref()?;
    let column = key_width + " = \"".width() + edit.input.before_cursor().width();
    Some((row, u16::try_from(column).unwrap_or(u16::MAX)))
}
