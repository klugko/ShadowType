//! Content of the editor pane, one module per kind of buffer.

mod doc;
mod forms;
mod help;
mod history;
mod room;
mod session;

use std::time::Instant;

use ratatui::{
    Frame,
    layout::{Position, Rect},
};

use crate::{
    app::{Activity, App, Buffer, mouse::Target},
    ui::{Moment, hits, theme::Palette},
};

/// Draws the selected buffer; returns where the terminal cursor goes, if anywhere.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
    now: Instant,
) -> Option<Position> {
    hits::mark(area, Target::Editor);
    match app.buffer {
        Buffer::Practice => forms::practice(frame, area, app, palette),
        Buffer::Race => forms::race(frame, area, app, palette),
        Buffer::Settings => forms::settings(frame, area, app, palette),
        Buffer::History => {
            history::render(frame, area, app, palette);
            None
        }
        Buffer::Help => {
            help::render(frame, area, app.help_scroll, palette);
            None
        }
        Buffer::Session => {
            match &app.activity {
                Some(Activity::Solo(run)) => {
                    let moment = Moment::of_app(app, now);
                    session::render(frame, area, app, run, palette, moment);
                }
                Some(Activity::Race(client)) => {
                    room::render(frame, area, app, client, palette, now)
                }
                None => {}
            }
            None
        }
    }
}

/**
 * Whether the buffer on screen moves at `moment`, its text aside: the
 * results of a solo session coming up.
 */
pub fn is_moving(app: &App, moment: Moment) -> bool {
    app.buffer == Buffer::Session
        && app
            .solo()
            .and_then(|run| run.result.as_ref())
            .is_some_and(|result| session::reveals(result, moment))
}
