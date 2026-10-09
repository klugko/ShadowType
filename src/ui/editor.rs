//! A buffer drawn like an editor window: line numbers, a highlighted
//! cursor line and `~` markers past the end of the file.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::mouse::Target,
    ui::{hits, theme::Palette},
};

/// One row of a buffer.
#[derive(Debug, Clone, Default)]
pub struct Row {
    /// Line number shown in the gutter, `None` for wrapped continuations.
    pub number: Option<usize>,
    pub spans: Vec<Span<'static>>,
    pub current: bool,
    /// Keys shown in the row that a click presses: their first column in
    /// the text, their width and what they press.
    pub keys: Vec<(u16, u16, Target)>,
}

impl Row {
    pub fn new(spans: Vec<Span<'static>>) -> Self {
        Self {
            number: None,
            spans,
            current: false,
            keys: Vec::new(),
        }
    }

    pub fn blank() -> Self {
        Self::default()
    }

    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }
}

/// Numbers rows from 1, skipping rows that already have a number.
pub fn number_rows(rows: &mut [Row]) {
    for (index, row) in rows.iter_mut().enumerate() {
        row.number.get_or_insert(index + 1);
    }
}

/// Width of the gutter for numbers up to `highest`.
pub fn gutter_width(highest: usize) -> u16 {
    let digits = highest.max(1).ilog10() + 1;
    u16::try_from(digits.max(3)).unwrap_or(3) + 2
}

/// Draws `rows` starting at `scroll`; returns the area used by the text.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    rows: &[Row],
    scroll: usize,
    palette: &Palette,
) -> Rect {
    let highest = rows.iter().filter_map(|row| row.number).max().unwrap_or(1);
    let gutter = gutter_width(highest);
    let lines: Vec<Line> = (0..usize::from(area.height))
        .map(|offset| match rows.get(scroll + offset) {
            Some(row) => buffer_line(row, gutter, palette),
            None if palette.tildes => Line::from(Span::styled("~", palette.fg(palette.faint))),
            None => Line::default(),
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(palette.base()), area);
    let text = Rect {
        x: area.x + gutter,
        width: area.width.saturating_sub(gutter),
        ..area
    };
    let shown = rows.iter().skip(scroll).take(usize::from(area.height));
    for (y, row) in (text.y..).zip(shown) {
        for (column, width, target) in &row.keys {
            let key = Rect::new(text.x.saturating_add(*column), y, *width, 1);
            hits::mark(key.intersection(text), *target);
        }
    }
    text
}

/// Text width available next to the gutter.
pub fn text_width(area_width: u16, highest_line: usize) -> u16 {
    area_width.saturating_sub(gutter_width(highest_line)).max(1)
}

/// First row to show so that `row` stays visible with a few rows of context
/// above it, like Vim's `scrolloff`, without scrolling past the end.
pub fn scroll_for(row: usize, height: u16, total: usize) -> usize {
    let height = usize::from(height).max(1);
    let context = (height / 3).min(2);
    row.saturating_sub(context)
        .min(total.saturating_sub(height))
}

fn buffer_line(row: &Row, gutter: u16, palette: &Palette) -> Line<'static> {
    let number = row
        .number
        .map_or_else(String::new, |number| number.to_string());
    let number_style = if row.current {
        palette.fg(palette.strong)
    } else {
        palette.fg(palette.faint)
    };
    let mut spans = vec![Span::styled(
        format!("{number:>width$}  ", width = usize::from(gutter) - 2),
        number_style,
    )];
    spans.extend(row.spans.iter().cloned());
    let line = Line::from(spans);
    if row.current {
        line.style(palette.cursorline)
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_grows_with_line_count() {
        assert_eq!(gutter_width(9), 5);
        assert_eq!(gutter_width(999), 5);
        assert_eq!(gutter_width(1_000), 6);
    }

    #[test]
    fn scroll_keeps_the_cursor_row_visible() {
        for total in [1, 10, 50, 200] {
            for height in [1, 5, 17] {
                for row in 0..total {
                    let scroll = scroll_for(row, height, total);
                    assert!(scroll <= row, "row {row} above view {scroll}");
                    assert!(row < scroll + usize::from(height), "row {row} below view");
                }
            }
        }
    }

    #[test]
    fn short_buffers_never_scroll() {
        assert_eq!(scroll_for(3, 17, 10), 0);
    }

    #[test]
    fn rows_are_numbered_from_one() {
        let mut rows = vec![Row::blank(), Row::blank()];
        number_rows(&mut rows);
        assert_eq!(rows[1].number, Some(2));
    }
}
