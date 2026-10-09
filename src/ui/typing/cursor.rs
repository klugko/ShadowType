use std::{f64::consts::TAU, time::Duration};

use code_racer_engine::Status;
use ratatui::style::Style;

use crate::{
    app::SessionView,
    ui::{
        Moment,
        theme::{Palette, blend},
    },
};

/// How long the cursor stays still after a key before it starts breathing.
pub(super) const BREATHE_AFTER: Duration = Duration::from_millis(600);
pub(super) const BREATH: Duration = Duration::from_millis(1_400);
/// How far the cursor fades into the line at the bottom of a breath.
const BREATH_DEPTH: f64 = 0.75;

/**
 * The cursor, breathing when the player pauses in a running text, in
 * themes where colours blend.
 */
pub(super) fn cursor_style(view: &SessionView<'_>, palette: &Palette, moment: Moment) -> Style {
    let still = palette.cursor;
    let (Some(ink), true, true) = (view.ink, moment.animate, palette.blends()) else {
        return still;
    };
    let (Some(last), Status::Running) = (ink.last_key(), view.session.status()) else {
        return still;
    };
    let pause = moment.now.saturating_duration_since(last);
    let Some(breathing) = pause.checked_sub(BREATHE_AFTER) else {
        return still;
    };
    let phase = (breathing.as_millis() % BREATH.as_millis()) as f64 / BREATH.as_millis() as f64;
    let fade = (1.0 - (phase * TAU).cos()) / 2.0 * BREATH_DEPTH;
    let (Some(fg), Some(bg)) = (still.fg, still.bg) else {
        return still;
    };
    let pending = palette.pending.fg.unwrap_or(fg);
    still
        .fg(blend(fg, pending, fade))
        .bg(blend(bg, palette.highlight, fade))
}
