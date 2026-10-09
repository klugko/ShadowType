//! A race room: lobby, race with live standings, and results.

mod lobby;
mod race;
mod results;

use std::time::Instant;

use code_racer_protocol::{PlayerId, PlayerView};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
};

use super::doc;
use crate::{
    app::{
        App,
        race::{RaceClient, Stage},
    },
    ui::{
        editor::{self, Row},
        format,
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
    match (&client.room, client.stage(now)) {
        (Some(room), Stage::Lobby) => lobby::render(frame, area, client, room, palette),
        (Some(room), Stage::Countdown(left)) => race::render(
            frame,
            area,
            app,
            room,
            Some(left.as_secs() + 1),
            palette,
            now,
        ),
        (Some(room), Stage::Racing) => race::render(frame, area, app, room, None, palette, now),
        (Some(room), Stage::Finished) => results::render(frame, area, client, room, palette),
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

/**
 * The rows of `players` that fit in `fitting` rows, made by `row` from each
 * player and their place in the list. The player `me` always shows, and a
 * last row counts the players left out.
 */
fn player_rows(
    players: &[&PlayerView],
    me: Option<PlayerId>,
    fitting: usize,
    palette: &Palette,
    row: impl Fn(usize, &PlayerView) -> Row,
) -> Vec<Row> {
    let shown = if players.len() > fitting {
        fitting.saturating_sub(1)
    } else {
        fitting
    };
    let visible = visible_players(players, me, shown);
    let hidden = players.len() - visible.len();
    let mut rows: Vec<Row> = visible
        .into_iter()
        .map(|(place, player)| row(place, player))
        .collect();
    if hidden > 0 {
        let more = format!("{hidden} more player{}", plural(hidden));
        rows.push(doc::comment(more, palette));
    }
    rows
}

/**
 * The places and players of the list of `players` that fit in `rows`
 * rows. The player `me` always shows: when their place falls below the
 * last row, they take that row, with their real place.
 */
fn visible_players<'a>(
    players: &[&'a PlayerView],
    me: Option<PlayerId>,
    rows: usize,
) -> Vec<(usize, &'a PlayerView)> {
    let mut visible: Vec<(usize, &PlayerView)> = players
        .iter()
        .copied()
        .enumerate()
        .map(|(index, player)| (index + 1, player))
        .take(rows)
        .collect();
    let mine = players.iter().position(|player| Some(player.id) == me);
    if let Some(index) = mine.filter(|index| *index >= rows)
        && let Some(last) = visible.last_mut()
    {
        *last = (index + 1, players[index]);
    }
    visible
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
        let visible = visible_players(&standings, Some(PlayerId(8)), 7);
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
            shown(&visible_players(&standings, Some(PlayerId(4)), 7)),
            all
        );
        assert_eq!(
            shown(&visible_players(&standings, Some(PlayerId(2)), 3)),
            all[..3],
            "the player is already shown"
        );
        assert_eq!(
            shown(&visible_players(&standings, None, 2)),
            all[..2],
            "a spectator sees the top"
        );
        assert!(visible_players(&standings, Some(PlayerId(4)), 0).is_empty());
    }

    #[test]
    fn a_list_too_long_ends_with_the_count_of_the_players_left_out() {
        let players = players(8);
        let list: Vec<&PlayerView> = players.iter().collect();
        let palette = Palette::of(crate::config::Theme::Editor);
        let texts = |fitting| -> Vec<String> {
            player_rows(&list, Some(PlayerId(8)), fitting, &palette, |place, _| {
                Row::new(vec![Span::raw(place.to_string())])
            })
            .iter()
            .map(|row| row.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect()
        };
        assert_eq!(texts(8), ["1", "2", "3", "4", "5", "6", "7", "8"]);
        assert_eq!(texts(5), ["1", "2", "3", "8", "# 4 more players"]);
        assert_eq!(texts(2), ["8", "# 7 more players"]);
    }
}
