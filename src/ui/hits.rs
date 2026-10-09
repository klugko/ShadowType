//! Where things can be clicked, as the screen is drawn. Rendering code
//! marks the regions it draws clickable things in; [`super::draw`] collects
//! them for the mouse.

use std::cell::RefCell;

use ratatui::layout::Rect;

use crate::app::mouse::{Hits, Target};

thread_local! {
    /// The regions marked while drawing the current frame.
    static MARKED: RefCell<Option<Hits>> = const { RefCell::new(None) };
}

/// Starts collecting the regions of a frame.
pub fn begin() {
    MARKED.with(|marked| *marked.borrow_mut() = Some(Hits::default()));
}

/// Marks `area` as `target`, over what was marked before, when a frame
/// is being collected.
pub fn mark(area: Rect, target: Target) {
    MARKED.with(|marked| {
        if let Some(hits) = marked.borrow_mut().as_mut() {
            hits.add(area, target);
        }
    });
}

/// The regions of the frame, which stops collecting.
pub fn finish() -> Hits {
    MARKED.with(|marked| marked.borrow_mut().take().unwrap_or_default())
}
