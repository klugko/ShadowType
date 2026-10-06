//! `help.md`: key bindings and commands.

use ratatui::{Frame, layout::Rect, style::Modifier, text::Span};

use super::doc;
use crate::{
    app::help::{LINES, Line},
    ui::{
        editor::{self, Row},
        theme::Palette,
    },
};

const KEY_WIDTH: usize = 20;

pub fn render(frame: &mut Frame, area: Rect, scroll: usize, palette: &Palette) {
    let mut rows: Vec<Row> = LINES.iter().map(|line| row(*line, palette)).collect();
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, scroll, palette);
}

fn row(line: Line, palette: &Palette) -> Row {
    match line {
        Line::Title(text) => doc::title(text, palette),
        Line::Heading(text) => doc::heading(text, palette),
        Line::Text(text) => doc::text(text, palette),
        Line::Blank => doc::blank(),
        Line::Key(keys, action) => Row::new(vec![
            Span::styled(
                format!("  {keys:<KEY_WIDTH$}"),
                palette.fg(palette.function).add_modifier(Modifier::BOLD),
            ),
            Span::styled(action, palette.fg(palette.text)),
        ]),
    }
}
