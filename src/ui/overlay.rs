//! The command palette, drawn over the editor like a code editor's: a line
//! to type in, and the commands that match what is typed under it.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{
        App,
        mouse::Target,
        palette::{CommandPalette, Match},
    },
    ui::{format, hits, theme::Palette},
};

const MAX_WIDTH: u16 = 76;
const MAX_LISTED: u16 = 12;
const PROMPT: &str = "> ";

/// Draws the palette at the top of `area`; returns where the terminal
/// cursor goes, after what is typed.
pub fn palette(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    open: &CommandPalette,
    palette: &Palette,
) -> Option<Position> {
    let found = app.palette_matches();
    let width = area.width.saturating_sub(4).min(MAX_WIDTH);
    let listed = u16::try_from(found.len())
        .unwrap_or(u16::MAX)
        .clamp(1, MAX_LISTED)
        .min(area.height.saturating_sub(5));
    let height = listed + 4;
    if width < 20 || height > area.height {
        return None;
    }
    let frame_area = Rect::new(area.x + (area.width - width) / 2, area.y, width, height);
    frame.render_widget(Clear, frame_area);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(palette.fg(palette.accent))
        .title(Span::styled(
            " commands ",
            palette.fg(palette.muted).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(
            Line::from(Span::styled(
                format!(" {} ", found.len()),
                palette.fg(palette.faint),
            ))
            .right_aligned(),
        )
        .style(Style::new().bg(palette.panel).fg(palette.text));
    let inner = block.inner(frame_area);
    frame.render_widget(block, frame_area);
    hits::mark(frame_area, Target::Overlay);

    let query_width = usize::from(inner.width).saturating_sub(PROMPT.width());
    let query = format::input_view(&open.query, query_width);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                PROMPT,
                palette.fg(palette.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(query.text, palette.fg(palette.strong)),
        ]),
        Line::from(Span::styled(
            "─".repeat(usize::from(inner.width)),
            palette.fg(palette.border),
        )),
    ];
    let selected = open.selected(found.len());
    let first = selected.saturating_sub(usize::from(listed).saturating_sub(1));
    for (index, found) in found
        .iter()
        .enumerate()
        .skip(first)
        .take(usize::from(listed))
    {
        let row = u16::try_from(index - first).unwrap_or(0);
        let y = inner.y + 2 + row;
        hits::mark(
            Rect::new(inner.x, y, inner.width, 1),
            Target::PaletteEntry(index),
        );
        lines.push(entry_line(found, index == selected, inner.width, palette));
    }
    if found.is_empty() {
        lines.push(Line::from(Span::styled(
            " no command matches",
            palette.fg(palette.muted),
        )));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    let column = PROMPT.width() + query.cursor;
    Some(Position::new(
        inner.x
            + u16::try_from(column)
                .unwrap_or(0)
                .min(inner.width.saturating_sub(1)),
        inner.y,
    ))
}

/// A command, the letters that match in bold, its shortcut on the right.
fn entry_line(found: &Match, selected: bool, width: u16, palette: &Palette) -> Line<'static> {
    let base = if selected {
        palette.selection
    } else {
        Style::new().fg(palette.text)
    };
    let width = usize::from(width);
    let shortcut = format!("{} ", found.entry.shortcut);
    let room = width.saturating_sub(shortcut.width() + 2);
    let title = format::truncate(&found.entry.title, room);
    let mut spans = vec![Span::styled(" ", base)];
    for (index, ch) in title.chars().enumerate() {
        let style = if found.positions.contains(&index) {
            base.fg(palette.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            base
        };
        spans.push(Span::styled(ch.to_string(), style));
    }
    let used = 1 + title.width();
    let gap = width.saturating_sub(used + shortcut.width());
    spans.push(Span::styled(" ".repeat(gap), base));
    if used + shortcut.width() <= width {
        let muted = if selected {
            base
        } else {
            palette.fg(palette.muted)
        };
        spans.push(Span::styled(shortcut, muted));
    }
    Line::from(spans)
}
