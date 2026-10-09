use code_racer_protocol::{PlayerView, RoomView};
use ratatui::{Frame, layout::Rect, style::Modifier, text::Span};

use super::{NAME_WIDTH, doc, name_span, player_rows};
use crate::{
    app::race::RaceClient,
    ui::{
        editor::{self, Row},
        format::{ordinal, race_time},
        theme::Palette,
    },
};

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    client: &RaceClient,
    room: &RoomView,
    palette: &Palette,
) {
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
    let footer = footer(client, room, palette);
    let fitting = usize::from(area.height).saturating_sub(rows.len() + footer.len());
    rows.extend(player_rows(
        &room.standings(),
        client.player,
        fitting,
        palette,
        |place, player| result_row(place, player, client, palette),
    ));
    rows.extend(footer);
    editor::number_rows(&mut rows);
    editor::render(frame, area, &rows, 0, palette);
}

/// The client's own result and what comes next.
fn footer(client: &RaceClient, room: &RoomView, palette: &Palette) -> [Row; 4] {
    let next = if client.is_host() {
        ("r", "back to the lobby for another race")
    } else {
        ("", "waiting for the host to start another race")
    };
    [
        doc::blank(),
        own_result(client, room, palette),
        doc::blank(),
        doc::keys(&[next, ("Esc Esc", "leave the room")], palette),
    ]
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
