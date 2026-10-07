//! A race room: lobby, race with live standings, and results.

use std::time::Instant;

use code_racer_protocol::{PlayerId, PlayerView, RoomView};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::{doc, session::text_rows};
use crate::{
    app::{
        App,
        race::{RaceClient, Stage},
    },
    ui::{
        editor::{self, Row},
        format::{self, ordinal, percent_done, race_time},
        theme::Palette,
    },
};

const NAME_WIDTH: usize = 16;
const LOBBY_KEY_WIDTH: usize = 7;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    client: &RaceClient,
    palette: &Palette,
    now: Instant,
) {
    match (&client.room, client.stage(now)) {
        (Some(room), Stage::Lobby) => lobby(frame, area, client, room, palette),
        (Some(room), Stage::Countdown(left)) => race(
            frame,
            area,
            app,
            client,
            room,
            Some(left.as_secs() + 1),
            palette,
        ),
        (Some(room), Stage::Racing) => race(frame, area, app, client, room, None, palette),
        (Some(room), Stage::Finished) => results(frame, area, client, room, palette),
        _ => connecting(frame, area, client, palette),
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

/// The lobby. The server address has a line of its own, so that it shows
/// in full when the invite command is too long for the window.
fn lobby(frame: &mut Frame, area: Rect, client: &RaceClient, room: &RoomView, palette: &Palette) {
    let setting = |key: &str, value: &str| {
        Row::new(doc::assignment(
            key,
            LOBBY_KEY_WIDTH,
            doc::string(value, palette),
            palette,
        ))
    };
    let mut rows = vec![doc::title(format!("room {}", room.code), palette)];
    if let Some(invite) = client.invite() {
        rows.push(doc::comment(format!("invite: {}", invite.command), palette));
        rows.extend(invite.note.map(|note| doc::comment(note, palette)));
    }
    rows.extend([
        doc::blank(),
        setting("server", &client.server),
        setting("text", &client.text_label()),
        setting(
            "players",
            &format!("{} / {}", room.players.len(), room.max_players),
        ),
        doc::blank(),
        doc::heading("players", palette),
    ]);
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
    if let Some(view) = app.session_view() {
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
    let lines: Vec<Line> =
        visible_standings(&room.standings(), client.player, usize::from(inner.height))
            .into_iter()
            .map(|(place, player)| standing_line(place, player, client, room, inner.width, palette))
            .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// The places and players of the `standings` that fit in `rows` rows.
/// The player `me` always shows: when their place falls below the last
/// row, they take that row, with their real place.
fn visible_standings<'a>(
    standings: &[&'a PlayerView],
    me: Option<PlayerId>,
    rows: usize,
) -> Vec<(usize, &'a PlayerView)> {
    let mut visible: Vec<(usize, &PlayerView)> = standings
        .iter()
        .copied()
        .enumerate()
        .map(|(index, player)| (index + 1, player))
        .take(rows)
        .collect();
    let mine = standings.iter().position(|player| Some(player.id) == me);
    if let Some(index) = mine.filter(|index| *index >= rows)
        && let Some(last) = visible.last_mut()
    {
        *last = (index + 1, standings[index]);
    }
    visible
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
    Span::styled(format::column(player.name.as_str(), NAME_WIDTH), style)
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use code_racer_protocol::PlayerProgress;

    use super::*;

    fn players(count: u64) -> Vec<PlayerView> {
        (1..=count)
            .map(|id| PlayerView {
                id: PlayerId(id),
                name: format!("racer{id}").parse().expect("name"),
                ready: true,
                connected: true,
                progress: PlayerProgress::default(),
            })
            .collect()
    }

    fn shown(visible: &[(usize, &PlayerView)]) -> Vec<(usize, u64)> {
        visible
            .iter()
            .map(|(place, player)| (*place, player.id.0))
            .collect()
    }

    #[test]
    fn the_player_last_of_a_full_room_takes_the_last_row() {
        let players = players(8);
        let standings: Vec<&PlayerView> = players.iter().collect();
        let visible = visible_standings(&standings, Some(PlayerId(8)), 7);
        assert_eq!(
            shown(&visible),
            [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6), (8, 8)]
        );
    }

    #[test]
    fn standings_that_fit_are_shown_as_they_are() {
        let players = players(4);
        let standings: Vec<&PlayerView> = players.iter().collect();
        let all = [(1, 1), (2, 2), (3, 3), (4, 4)];
        assert_eq!(
            shown(&visible_standings(&standings, Some(PlayerId(4)), 7)),
            all
        );
        assert_eq!(
            shown(&visible_standings(&standings, Some(PlayerId(2)), 3)),
            all[..3],
            "the player is already shown"
        );
        assert_eq!(
            shown(&visible_standings(&standings, None, 2)),
            all[..2],
            "a spectator sees the top"
        );
        assert!(visible_standings(&standings, Some(PlayerId(4)), 0).is_empty());
    }
}
