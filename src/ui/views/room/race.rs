use std::time::Instant;

use code_racer_protocol::{PlayerView, RoomView};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::{NAME_WIDTH, name_span, visible_players};
use crate::{
    app::{App, race::RaceClient},
    ui::{
        Moment, editor,
        format::{percent_done, race_time},
        theme::Palette,
        views::session::text_rows,
    },
};

/// The race of the room of `app`: its text, the standings under it.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    room: &RoomView,
    countdown: Option<u64>,
    palette: &Palette,
    now: Instant,
) {
    let Some(client) = app.race() else {
        return;
    };
    let panel_height = u16::try_from(room.players.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height / 2);
    let [text_area, panel_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(panel_height)]).areas(area);
    if let Some(view) = app.session_view() {
        let active = countdown.is_none() && app.is_typing();
        let moment = Moment::of_app(app, now);
        let (rows, cursor_row) = text_rows(&view, text_area, palette, active, moment);
        let scroll = editor::scroll_for(cursor_row, text_area.height, rows.len());
        editor::render(frame, text_area, &rows, scroll, palette);
    }
    standings(frame, panel_area, client, room, countdown, palette);
}

fn standings(
    frame: &mut Frame,
    area: Rect,
    client: &RaceClient,
    room: &RoomView,
    countdown: Option<u64>,
    palette: &Palette,
) {
    let title = match countdown {
        Some(seconds) => format!(" PLAYERS · starting in {seconds} "),
        None => format!(" PLAYERS · {} ", room.code),
    };
    let title_style = match countdown {
        Some(_) => palette.fg(palette.warning).add_modifier(Modifier::BOLD),
        None => palette.fg(palette.muted).add_modifier(Modifier::BOLD),
    };
    let block = Block::new()
        .borders(Borders::TOP)
        .border_style(palette.fg(palette.border))
        .title(Span::styled(title, title_style))
        .style(palette.base());
    let inner = block.inner(area);
    let lines: Vec<Line> =
        visible_players(&room.standings(), client.player, usize::from(inner.height))
            .into_iter()
            .map(|(place, player)| standing_line(place, player, client, room, inner.width, palette))
            .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn standing_line(
    place: usize,
    player: &PlayerView,
    client: &RaceClient,
    room: &RoomView,
    width: u16,
    palette: &Palette,
) -> Line<'static> {
    let progress = player.progress;
    let status = if let Some(ms) = progress.finish_ms {
        Span::styled(format!("✓ {}", race_time(ms)), palette.fg(palette.success))
    } else if !player.connected {
        Span::styled("offline".to_owned(), palette.fg(palette.muted))
    } else {
        Span::raw(String::new())
    };
    let fixed = 4 + NAME_WIDTH + 2 + 5 + 9 + 12;
    let bar_width = usize::from(width).saturating_sub(fixed).clamp(8, 48);
    let percent = percent_done(progress.fraction(room.text_length));
    let filled = bar_width * percent as usize / 100;
    let bar_color = if Some(player.id) == client.player {
        palette.insert
    } else {
        palette.accent
    };
    Line::from(vec![
        Span::styled(format!("{place:>3} "), palette.fg(palette.faint)),
        name_span(player, client, palette),
        Span::raw("  "),
        Span::styled("━".repeat(filled), palette.fg(bar_color)),
        Span::styled("─".repeat(bar_width - filled), palette.fg(palette.faint)),
        Span::styled(format!("{percent:>4}%"), palette.fg(palette.text)),
        Span::styled(
            format!("{:>5.0} wpm", progress.wpm),
            palette.fg(palette.number),
        ),
        Span::raw("  "),
        status,
    ])
}
