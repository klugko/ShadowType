use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use super::{project_name, spans_width};
use crate::{
    app::{App, Buffer, mouse::Target},
    ui::{hits, icons, theme::Palette},
};

pub fn tabline(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) {
    let mut tabs = vec![app.buffer];
    if app.activity.is_some() && app.buffer != Buffer::Session {
        tabs.push(Buffer::Session);
    }
    let mut spans = Vec::new();
    for buffer in tabs {
        let tab = tab(app, buffer, palette);
        let x = area
            .x
            .saturating_add(u16::try_from(spans_width(&spans)).unwrap_or(u16::MAX));
        let width = u16::try_from(spans_width(&tab)).unwrap_or(u16::MAX);
        hits::mark(
            Rect::new(x, area.y, width, 1).intersection(area),
            Target::Tab(buffer),
        );
        spans.extend(tab);
        spans.push(Span::styled(
            "│",
            palette.fg(palette.border).bg(palette.panel),
        ));
    }
    let brand = format!(" {} ", project_name(app));
    let filler = usize::from(area.width).saturating_sub(spans_width(&spans) + brand.width());
    spans.push(Span::styled(
        " ".repeat(filler),
        Style::new().bg(palette.panel),
    ));
    spans.push(Span::styled(
        brand,
        Style::new().bg(palette.panel).fg(palette.faint),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn tab(app: &App, buffer: Buffer, palette: &Palette) -> Vec<Span<'static>> {
    let name = app.buffer_name(buffer);
    let modified = if buffer == Buffer::Session && app.session_in_progress() {
        " ●"
    } else {
        ""
    };
    let style = if buffer == app.buffer {
        Style::new()
            .bg(palette.background)
            .fg(palette.strong)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().bg(palette.panel).fg(palette.text)
    };
    match icons::file(&name, app.config.icons, palette) {
        Some((glyph, color)) => vec![
            Span::styled(" ", style),
            Span::styled(glyph, style.fg(color)),
            Span::styled(format!(" {name}{modified} "), style),
        ],
        None => vec![Span::styled(format!(" {name}{modified} "), style)],
    }
}
