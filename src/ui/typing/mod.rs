/*!
 * The text being typed: ghost text ahead of the cursor, real text behind it.
 * Code shows its syntax from the start, dimmed until typed; prose is dressed
 * as the kind of file its look makes it.
 */

mod code;
mod cursor;
mod painter;

use code_racer_engine::Status;
use ratatui::{style::Modifier, text::Span};

use crate::{
    app::SessionView,
    ui::{Moment, editor::Row, looks, theme::Palette, wrap},
};
use code::Syntax;
use cursor::cursor_style;
use painter::Painter;

#[derive(Debug)]
pub struct TypingLayout {
    pub rows: Vec<Row>,
    pub cursor_row: usize,
}

/**
 * Prose is numbered row by row like a soft-wrapped file, with the lines its
 * look puts around it; code keeps its own line numbers. The cursor block is
 * drawn only while `active`.
 */
pub fn layout(
    view: &SessionView<'_>,
    width: u16,
    palette: &Palette,
    active: bool,
    moment: Moment,
) -> TypingLayout {
    let target = view.session.target();
    let prose = !target.iter().any(|grapheme| grapheme == "\n");
    let disguise = view.disguise.filter(|_| view.syntax.is_none());
    let prefix = disguise.map_or(0, looks::prefix_width);
    let lines = wrap::wrap(target, width.saturating_sub(prefix).max(1));
    let painter = Painter {
        view,
        palette,
        moment,
        disguise,
        cursor: active.then(|| view.session.cursor()),
        cursor_style: cursor_style(view, palette, moment),
        syntax: view.syntax.map(|language| Syntax::of(target, language)),
        indent: (!prose).then(|| code::indent_unit(target, &lines)),
    };
    let (cursor_line, _) = wrap::locate(target, &lines, view.session.cursor());
    let mut rows = disguise.map_or_else(Vec::new, |disguise| looks::header(disguise, palette));
    let cursor_row = rows.len() + cursor_line;
    for (index, line) in lines.iter().enumerate() {
        let last = index + 1 == lines.len();
        rows.push(painter.row(index, line, index == cursor_line, last));
    }
    if let Some(source) = view.attribution {
        rows.extend(attribution(source, palette));
    }
    if let Some(disguise) = disguise {
        rows.extend(looks::footer(disguise, palette));
    }
    if prose {
        for (number, row) in (1..).zip(&mut rows) {
            row.number = Some(number);
        }
    }
    TypingLayout { rows, cursor_row }
}

/**
 * Whether the text of `view` moves at `moment`: fresh ink drying, the
 * cursor trailing, or a running text, whose cursor breathes.
 */
pub fn is_moving(view: &SessionView<'_>, moment: Moment) -> bool {
    let Some(ink) = view.ink.filter(|_| moment.animate) else {
        return false;
    };
    ink.is_wet(moment.now)
        || (moment.trail && ink.is_trailing(moment.now))
        || view.session.status() == Status::Running
}

fn attribution(source: &str, palette: &Palette) -> [Row; 2] {
    [
        Row::blank(),
        Row::new(vec![Span::styled(
            format!("-- {source}"),
            palette.fg(palette.comment).add_modifier(Modifier::ITALIC),
        )]),
    ]
}

#[cfg(test)]
mod tests;
