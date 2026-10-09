/*!
 * Rendering. The layout mimics a code editor: explorer on the left, tab
 * line and buffer on the right, status line and command line at the bottom.
 */

mod chart;
mod chrome;
mod editor;
mod format;
mod hits;
mod icons;
mod looks;
mod mascot;
mod overlay;
mod syntax;
mod theme;
mod typing;
mod views;
mod wrap;

use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::app::{App, Buffer, mascot::Mood, mouse::Hits};
use theme::Palette;

/// When a frame is drawn, and whether what moves on screen may move in it.
#[derive(Debug, Clone, Copy)]
pub struct Moment {
    pub now: Instant,
    pub animate: bool,
    pub trail: bool,
}

impl Moment {
    #[cfg(test)]
    pub fn still(now: Instant) -> Self {
        Self {
            now,
            animate: false,
            trail: false,
        }
    }

    pub fn moving(now: Instant) -> Self {
        Self {
            now,
            animate: true,
            trail: true,
        }
    }

    pub fn of_app(app: &App, now: Instant) -> Self {
        Self {
            now,
            animate: app.config.animations,
            trail: app.config.animations && app.config.trail,
        }
    }
}

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 20;
const SIDEBAR_WIDTH: u16 = 24;
/**
 * How often the screen is redrawn while text moves on it: ink drying, the
 * cursor breathing.
 */
const TEXT_FRAME: Duration = Duration::from_millis(40);
/// How often the screen is redrawn while only the mascot moves.
const MASCOT_FRAME: Duration = Duration::from_millis(100);

/// Draws `app` at `now`; returns where the things a click acts on lie.
pub fn draw(frame: &mut Frame, app: &App, now: Instant) -> Hits {
    hits::begin();
    draw_screen(frame, app, now);
    hits::finish()
}

fn draw_screen(frame: &mut Frame, app: &App, now: Instant) {
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
        Constraint::Length(chrome::cmdline_height(app, area.width)),
    ])
    .areas(area);
    let main = if app.sidebar {
        let [sidebar, main] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(1)]).areas(body);
        chrome::sidebar(frame, sidebar, app, &palette, Moment::of_app(app, now));
        main
    } else {
        body
    };
    let [tabs, buffer] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(main);
    chrome::tabline(frame, tabs, app, &palette);
    let field_cursor = views::render(frame, buffer, app, &palette, now);
    let overlay_cursor = app
        .palette
        .as_ref()
        .and_then(|open| overlay::palette(frame, buffer, app, open, &palette));
    chrome::statusline(frame, status, app, &palette, now);
    let prompt_cursor = chrome::cmdline(frame, command, app, &palette);
    if let Some(position) = prompt_cursor.or(overlay_cursor).or(field_cursor) {
        frame.set_cursor_position(position);
    }
}

/**
 * How soon the screen has to be drawn again for what moves on it to move,
 * `None` while nothing moves: animations are off, or the text is still and
 * the mascot asleep or out of sight.
 */
pub fn frame_period(app: &App, now: Instant) -> Option<Duration> {
    if !app.config.animations {
        return None;
    }
    let moment = Moment::moving(now);
    let text_moves = app.buffer == Buffer::Session
        && app
            .session_view()
            .is_some_and(|view| typing::is_moving(&view, moment));
    if text_moves || views::is_moving(app, moment) {
        return Some(TEXT_FRAME);
    }
    (chrome::shows_mascot(app) && app.mascot_mood(now) != Mood::Asleep).then_some(MASCOT_FRAME)
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
