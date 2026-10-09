/*!
 * The mascot: a pixel-art ghost under the explorer that reacts to the typing.
 *
 * It is drawn with half blocks, two pixels to a cell: `▀` takes the colour
 * of its upper pixel as foreground and of its lower pixel as background.
 */

mod pose;
mod sprite;

use std::time::Duration;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app::mascot::Mood, ui::theme::Palette};
use pose::Pose;
use sprite::CANVAS_WIDTH;

/// Rows the mascot takes.
pub const HEIGHT: u16 = 7;
/// Columns the mascot takes, the sparkles around it included.
pub const WIDTH: u16 = 20;
/// Column of the sprite in the area of the mascot.
const LEFT: usize = 2;
/// The age the mascot is drawn at when nothing moves: its eyes open.
pub const STILL: Duration = Duration::from_secs(1);

pub fn render(frame: &mut Frame, area: Rect, mood: Mood, age: Duration, palette: &Palette) {
    let canvas = sprite::canvas(Pose::of(mood, age));
    let mut lines: Vec<Vec<Span<'static>>> = (0..HEIGHT)
        .map(|row| {
            let mut spans: Vec<Span<'static>> = (0..LEFT)
                .map(|_| Span::styled(" ", panel(palette)))
                .collect();
            spans.extend(sprite::cells(&canvas, usize::from(row), palette));
            spans
        })
        .collect();
    for (column, row, text, color) in decorations(mood, age, palette) {
        if let Some(line) = lines.get_mut(row) {
            put(line, LEFT + CANVAS_WIDTH + column, text, palette.fg(color));
        }
    }
    let lines: Vec<Line> = lines.into_iter().map(Line::from).collect();
    frame.render_widget(Paragraph::new(lines).style(panel(palette)), area);
}

/**
 * Sparkles and snores around the ghost: column after the canvas, row, text
 * and colour.
 */
fn decorations(
    mood: Mood,
    age: Duration,
    palette: &Palette,
) -> Vec<(usize, usize, &'static str, Color)> {
    let beat = (age.as_millis() / 300).is_multiple_of(2);
    match mood {
        Mood::Asleep => vec![
            (0, 3, "z", palette.muted),
            (1, 2, "z", palette.muted),
            (2, 1, "Z", palette.text),
        ],
        Mood::Oops => vec![(0, 1, "!", palette.warning)],
        Mood::Typing => {
            let dots =
                ["·  ", "·· ", "···"][usize::try_from(age.as_millis() / 400 % 3).unwrap_or(0)];
            vec![(0, 4, dots, palette.muted)]
        }
        Mood::Happy if beat => vec![(0, 1, "✦", palette.warning), (2, 3, "·", palette.accent)],
        Mood::Happy => vec![(1, 0, "·", palette.warning), (0, 3, "✧", palette.accent)],
        Mood::Proud if beat => vec![
            (0, 1, "✦", palette.warning),
            (2, 3, "✧", palette.accent),
            (1, 5, "·", palette.success),
        ],
        Mood::Proud => vec![
            (1, 0, "✧", palette.warning),
            (0, 3, "✦", palette.accent),
            (2, 4, "✦", palette.success),
        ],
        Mood::Idle => Vec::new(),
    }
}

/// Writes `text` at `column` of a row of one-column spans.
fn put(line: &mut Vec<Span<'static>>, column: usize, text: &str, style: Style) {
    while line.len() < column {
        line.push(Span::raw(" "));
    }
    let span = Span::styled(text.to_owned(), style);
    if column < line.len() {
        line[column] = span;
    } else {
        line.push(span);
    }
}

fn panel(palette: &Palette) -> Style {
    Style::new().bg(palette.panel)
}
