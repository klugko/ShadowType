use code_racer_protocol::{PlayerView, RoomView};
use ratatui::{Frame, layout::Rect, text::Span};

use super::{doc, name_span, player_rows, plural};
use crate::{
    app::race::RaceClient,
    ui::{
        editor::{self, Row},
        theme::Palette,
    },
};

const KEY_WIDTH: usize = 7;

/**
 * The lobby. The server address has a line of its own, so that it shows
 * in full when the invite command is too long for the window.
 */
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    client: &RaceClient,
    room: &RoomView,
    palette: &Palette,
) {
    let setting = |key: &str, value: &str| {
        Row::new(doc::assignment(
            key,
            KEY_WIDTH,
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
    let footer = footer(client, room, palette);
    let players: Vec<&PlayerView> = room.players.iter().collect();
    let fitting = usize::from(area.height).saturating_sub(rows.len() + footer.len());
    rows.extend(player_rows(
        &players,
        client.player,
        fitting,
        palette,
        |_, player| player_row(player, client, room, palette),
    ));
    rows.extend(footer);
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

fn player_row(player: &PlayerView, client: &RaceClient, room: &RoomView, palette: &Palette) -> Row {
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

/// What to do next in the lobby: wait or start, and the keys.
fn footer(client: &RaceClient, room: &RoomView, palette: &Palette) -> [Row; 4] {
    let mut keys = vec![("r", "toggle ready")];
    if client.is_host() {
        keys.push(("s", "start the race"));
    }
    keys.push(("Esc", "leave"));
    [
        doc::blank(),
        hint(client, room, palette),
        doc::blank(),
        doc::keys(&keys, palette),
    ]
}

fn hint(client: &RaceClient, room: &RoomView, palette: &Palette) -> Row {
    let waiting = room.players.iter().filter(|player| !player.ready).count();
    let hint = match (client.is_host(), waiting) {
        (true, 0) => "everyone is ready, press s to start".to_owned(),
        (false, 0) => "everyone is ready, waiting for the host to start".to_owned(),
        (_, count) => format!("waiting for {count} player{} to get ready", plural(count)),
    };
    doc::comment(hint, palette)
}
