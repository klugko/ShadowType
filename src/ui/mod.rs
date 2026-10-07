//! Rendering. The layout mimics a code editor: explorer on the left, tab
//! line and buffer on the right, status line and command line at the bottom.

mod chart;
mod chrome;
mod editor;
mod format;
mod syntax;
mod theme;
mod typing;
mod views;
mod wrap;

use std::time::Instant;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::app::App;
use theme::Palette;

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 20;
const SIDEBAR_WIDTH: u16 = 24;

pub fn draw(frame: &mut Frame, app: &App, now: Instant) {
    let palette = Palette::of(app.config.theme);
    let area = frame.area();
    frame.render_widget(Block::new().style(palette.base()), area);
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        too_small(frame, area, &palette);
        return;
    }
    let [body, status, command] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let main = if app.sidebar {
        let [sidebar, main] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(1)]).areas(body);
        chrome::sidebar(frame, sidebar, app, &palette);
        main
    } else {
        body
    };
    let [tabs, buffer] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(main);
    chrome::tabline(frame, tabs, app, &palette);
    let field_cursor = views::render(frame, buffer, app, &palette, now);
    chrome::statusline(frame, status, app, &palette, now);
    let prompt_cursor = chrome::cmdline(frame, command, app, &palette);
    if let Some(position) = prompt_cursor.or(field_cursor) {
        frame.set_cursor_position(position);
    }
}

fn too_small(frame: &mut Frame, area: Rect, palette: &Palette) {
    let lines = vec![
        Line::from(Span::styled(
            "Terminal too small.",
            palette.fg(palette.strong).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Minimum recommended size: {MIN_WIDTH}x{MIN_HEIGHT}"),
            palette.fg(palette.muted),
        )),
        Line::from(Span::styled(
            format!("Current size: {}x{}", area.width, area.height),
            palette.fg(palette.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

#[cfg(test)]
mod tests;
