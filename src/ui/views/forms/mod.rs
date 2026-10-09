//! `practice.toml`, `race.toml` and `config.toml`: settings shown as TOML.

mod field;

use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::Span,
};

use super::doc;
use crate::{
    app::{App, Focus, TextField, form, input::TextInput, mouse::Target, practice, race, settings},
    ui::{
        editor::{self, Row},
        hits,
        theme::Palette,
    },
};
use field::{Columns, field_row};

/// A line of a form document.
enum Item<'a> {
    Plain(Row),
    Field {
        row: form::Row,
        selected: bool,
        editing: Option<&'a TextInput>,
    },
}

impl<'a> Item<'a> {
    /**
     * A form line of `app`, being typed when it takes `text` and that text
     * is being edited.
     */
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
    let items = fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            Item::field(
                app,
                practice::row(&app.config, *field),
                None,
                index == selected,
            )
        })
        .collect();
    draw(frame, area, items, palette)
}

pub fn race(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    let fields = race::fields(&app.config.race);
    let selected = app.race_cursor.index(fields.len());
    let mut items = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if let Some(section) = race::section(*field) {
            if !items.is_empty() {
                items.push(Item::Plain(doc::blank()));
            }
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
    let mut items = Vec::new();
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

/**
 * Returns where the terminal cursor goes while a value is being typed and
 * in view.
 */
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
    let mut lines = Vec::new();
    for item in items {
        let row = match item {
            Item::Plain(row) => row,
            Item::Field {
                row,
                selected,
                editing,
            } => {
                lines.push(rows.len());
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
    mark_lines(&lines, scroll, area);
    let (row, column) = cursor?;
    let row = u16::try_from(row.checked_sub(scroll)?).ok()?;
    let column = u16::try_from(column).unwrap_or(u16::MAX);
    (row < text.height).then_some(Position {
        x: text.x + column.min(text.width.saturating_sub(1)),
        y: text.y + row,
    })
}

/// `lines` holds the row of each form line, from the top of the document.
fn mark_lines(lines: &[usize], scroll: usize, area: Rect) {
    for (index, row) in lines.iter().enumerate() {
        let shown = row
            .checked_sub(scroll)
            .and_then(|offset| u16::try_from(offset).ok())
            .filter(|offset| *offset < area.height);
        if let Some(offset) = shown {
            let line = Rect::new(area.x, area.y + offset, area.width, 1);
            hits::mark(line, Target::FormLine(index));
        }
    }
}
