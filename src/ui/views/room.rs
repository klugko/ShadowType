//! A race room: lobby, race with live standings, and results.

use std::time::Instant;

use code_racer_protocol::{PlayerView, RoomView};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{doc, ordinal, race_time, session::text_rows};
use crate::{
    app::{
        App, SessionView,
        race::{RaceClient, Stage},
    },
    ui::{
        editor::{self, Row},
        theme::Palette,
    },
};

const NAME_WIDTH: usize = 16;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    client: &RaceClient,
    palette: &Palette,
    now: Instant,
) {
    let Some(room) = &client.room else {
        connecting(frame, area, client, palette);
        return;
    };
    match client.stage(now) {
        Stage::Connecting | Stage::Joining => connecting(frame, area, client, palette),
        Stage::Lobby => lobby(frame, area, client, room, palette),
        Stage::Countdown(left) => race(
            frame,
            area,
            app,
            client,
            room,
            Some(left.as_secs() + 1),
            palette,
        ),
        Stage::Racing => race(frame, area, app, client, room, None, palette),
        Stage::Finished => results(frame, area, client, room, palette),
    }
}

fn connecting(frame: &mut Frame, area: Rect, client: &RaceClient, palette: &Palette) {
    let mut rows = vec![
        doc::title("connecting", palette),
        doc::blank(),
        Row::new(doc::assignment(
            "server",
            6,
            doc::string(&client.server, palette),
            palette,
        )),
        doc::blank(),
        doc::comment("waiting for the race server, Esc cancels", palette),
    ];
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

fn lobby(frame: &mut Frame, area: Rect, client: &RaceClient, room: &RoomView, palette: &Palette) {
    let mut rows = vec![
        doc::title(format!("room {}", room.code), palette),
        doc::comment(
            format!(
                "invite: code-racer join {} --server {}",
                room.code, client.server
            ),
            palette,
        ),
        doc::blank(),
        Row::new(doc::assignment(
            "text",
            7,
            doc::string(&client.text_label(), palette),
            palette,
        )),
        Row::new(doc::assignment(
            "players",
            7,
            doc::string(
                &format!("{} / {}", room.players.len(), room.max_players),
                palette,
            ),
            palette,
        )),
        doc::blank(),
        doc::heading("players", palette),
    ];
    rows.extend(
        room.players
            .iter()
            .map(|player| lobby_player(player, client, room, palette)),
    );
    rows.push(doc::blank());
    rows.push(lobby_hint(client, room, palette));
    rows.push(doc::blank());
    let mut keys = vec![("r", "toggle ready")];
    if client.is_host() {
        keys.push(("s", "start the race"));
    }
    keys.push(("Esc", "leave"));
    rows.push(doc::keys(&keys, palette));
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

fn lobby_player(
    player: &PlayerView,
    client: &RaceClient,
    room: &RoomView,
    palette: &Palette,
) -> Row {
    let (mark, state) = if player.ready {
        (
            Span::styled("✓ ", palette.fg(palette.success)),
            Span::styled("ready", palette.fg(palette.success)),
        )
    } else {
        (
            Span::styled("· ", palette.fg(palette.muted)),
            Span::styled("not ready", palette.fg(palette.muted)),
        )
    };
    let mut tags = Vec::new();
    if room.is_host(player.id) {
        tags.push("host");
    }
    if Some(player.id) == client.player {
        tags.push("you");
    }
    Row::new(vec![
        Span::raw("  "),
        mark,
        name_span(player, client, palette),
        Span::styled(
            format!("{:<12}", tags.join(", ")),
            palette.fg(palette.comment),
        ),
        state,
    ])
}

fn lobby_hint(client: &RaceClient, room: &RoomView, palette: &Palette) -> Row {
    let waiting = room.players.iter().filter(|player| !player.ready).count();
    let hint = match (client.is_host(), waiting) {
        (true, 0) => "everyone is ready, press s to start".to_owned(),
        (false, 0) => "everyone is ready, waiting for the host to start".to_owned(),
        (_, count) => format!("waiting for {count} player{} to get ready", plural(count)),
    };
    doc::comment(hint, palette)
}

fn race(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    client: &RaceClient,
    room: &RoomView,
    countdown: Option<u64>,
    palette: &Palette,
) {
    let panel_height = u16::try_from(room.players.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height / 2);
    let [text_area, panel_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(panel_height)]).areas(area);
    if let Some(race) = &client.race {
        let view = SessionView {
            session: &race.session,
            syntax: client.syntax(),
            attribution: None,
        };
        let active = countdown.is_none() && app.is_typing();
        let (rows, cursor_row) = text_rows(&view, text_area, palette, active);
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
    let lines: Vec<Line> = room
        .standings()
        .into_iter()
        .enumerate()
        .map(|(index, player)| standing_line(index + 1, player, client, room, inner.width, palette))
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
    let fraction = progress.fraction(room.text_length);
    let filled = ((fraction * bar_width as f64).round() as usize).min(bar_width);
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
        Span::styled(
            format!("{:>4.0}%", fraction * 100.0),
            palette.fg(palette.text),
        ),
        Span::styled(
            format!("{:>5.0} wpm", progress.wpm),
            palette.fg(palette.number),
        ),
        Span::raw("  "),
        status,
    ])
}

fn results(frame: &mut Frame, area: Rect, client: &RaceClient, room: &RoomView, palette: &Palette) {
    let mut rows = vec![
        doc::title(format!("results · room {}", room.code), palette),
        doc::comment(client.text_label(), palette),
        doc::blank(),
        Row::new(vec![Span::styled(
            format!(
                "{:>3}  {:<NAME_WIDTH$}{:>8}{:>11}{:>10}",
                "#", "player", "wpm", "accuracy", "time"
            ),
            palette.fg(palette.muted).add_modifier(Modifier::BOLD),
        )]),
    ];
    for (index, player) in room.standings().into_iter().enumerate() {
        rows.push(result_row(index + 1, player, client, palette));
    }
    rows.push(doc::blank());
    rows.push(own_result(client, room, palette));
    rows.push(doc::blank());
    let mut keys = Vec::new();
    if client.is_host() {
        keys.push(("r", "back to the lobby for another race"));
    } else {
        keys.push(("", "waiting for the host to start another race"));
    }
    keys.push(("Esc", "leave"));
    rows.push(doc::keys(&keys, palette));
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

fn result_row(place: usize, player: &PlayerView, client: &RaceClient, palette: &Palette) -> Row {
    let progress = player.progress;
    let (wpm, accuracy, time) = match progress.finish_ms {
        Some(ms) => (
            format!("{:.1}", progress.wpm),
            format!("{:.1}%", progress.accuracy),
            race_time(ms),
        ),
        None => ("-".to_owned(), "-".to_owned(), "DNF".to_owned()),
    };
    Row::new(vec![
        Span::styled(format!("{place:>3}  "), palette.fg(palette.faint)),
        name_span(player, client, palette),
        Span::styled(format!("{wpm:>8}"), palette.fg(palette.number)),
        Span::styled(format!("{accuracy:>11}"), palette.fg(palette.number)),
        Span::styled(format!("{time:>10}"), palette.fg(palette.text)),
    ])
}

fn own_result(client: &RaceClient, room: &RoomView, palette: &Palette) -> Row {
    let Some(me) = client.me() else {
        return doc::blank();
    };
    let place = client.player.and_then(|id| room.place_of(id)).unwrap_or(0);
    match me.progress.finish_ms {
        Some(_) => Row::new(vec![Span::styled(
            format!(
                "you finished {} of {} with {:.0} wpm",
                ordinal(place),
                room.players.len(),
                me.progress.wpm
            ),
            palette.fg(palette.success).add_modifier(Modifier::BOLD),
        )]),
        None => doc::comment("you did not finish this race", palette),
    }
}

fn name_span(player: &PlayerView, client: &RaceClient, palette: &Palette) -> Span<'static> {
    let style = if Some(player.id) == client.player {
        palette.fg(palette.strong).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(palette.text)
    };
    Span::styled(fit(player.name.as_str(), NAME_WIDTH), style)
}

/// Pads or truncates `text` to exactly `width` columns.
fn fit(text: &str, width: usize) -> String {
    let mut fitted = String::new();
    for grapheme in text.graphemes(true) {
        if fitted.width() + grapheme.width() > width.saturating_sub(1) {
            break;
        }
        fitted.push_str(grapheme);
    }
    let padding = width.saturating_sub(fitted.width());
    fitted + &" ".repeat(padding)
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_fitted_to_their_column() {
        assert_eq!(fit("jean", 8), "jean    ");
        assert_eq!(fit("a-very-long-name", 8).width(), 8);
        assert_eq!(fit("été", 5), "été  ");
    }
}
