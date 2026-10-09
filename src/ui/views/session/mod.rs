//! A solo session: the text while typing, then the results.

mod results;

use ratatui::{Frame, layout::Rect};

use crate::{
    app::{App, SessionView, practice::SoloRun},
    ui::{
        Moment,
        editor::{self, Row},
        theme::Palette,
        typing,
    },
};
pub use results::reveals;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    run: &SoloRun,
    palette: &Palette,
    moment: Moment,
) {
    match &run.result {
        Some(result) => results::render(frame, area, run, result, palette, moment),
        None => text(frame, area, app, palette, moment),
    }
}

fn text(frame: &mut Frame, area: Rect, app: &App, palette: &Palette, moment: Moment) {
    let Some(view) = app.session_view() else {
        return;
    };
    let (rows, cursor_row) = text_rows(&view, area, palette, app.is_typing(), moment);
    let scroll = editor::scroll_for(cursor_row, area.height, rows.len());
    editor::render(frame, area, &rows, scroll, palette);
}

/**
 * Rows of a session text laid out for `area` at `moment`, and the row of
 * the cursor.
 */
pub fn text_rows(
    view: &SessionView<'_>,
    area: Rect,
    palette: &Palette,
    active: bool,
    moment: Moment,
) -> (Vec<Row>, usize) {
    let mut highest = 999;
    loop {
        let width = editor::text_width(area.width, highest);
        let layout = typing::layout(view, width, palette, active, moment);
        if layout.rows.len() <= highest {
            return (layout.rows, layout.cursor_row);
        }
        highest = layout.rows.len();
    }
}
