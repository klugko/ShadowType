use ratatui::{
    style::{Modifier, Style},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use super::{RowState, glowing};
use crate::ui::{Moment, editor::Row, theme::Palette};

pub(super) fn prefix_width() -> usize {
    "- [ ] ".width()
}

pub(super) fn header(palette: &Palette) -> Vec<Row> {
    let title = Row::new(vec![Span::styled(
        "# TODO",
        palette.fg(palette.keyword).add_modifier(Modifier::BOLD),
    )]);
    vec![title, Row::blank()]
}

pub(super) fn prefix(row: RowState, palette: &Palette, moment: Moment) -> Vec<Span<'static>> {
    let (mark, color) = if row.done {
        ("x", glowing(palette, palette.success, row.done_at, moment))
    } else {
        (" ", palette.muted)
    };
    vec![
        Span::styled("- [", palette.fg(palette.punctuation)),
        Span::styled(mark, palette.fg(color).add_modifier(Modifier::BOLD)),
        Span::styled("] ", palette.fg(palette.punctuation)),
    ]
}

pub(super) fn restyle(row: RowState, style: Style, palette: &Palette) -> Style {
    match (row.done, palette.mono) {
        (false, _) => style,
        (true, false) => style.fg(palette.muted).add_modifier(Modifier::CROSSED_OUT),
        (true, true) => style.add_modifier(Modifier::CROSSED_OUT),
    }
}
