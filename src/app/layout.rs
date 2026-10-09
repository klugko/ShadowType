use super::{App, Buffer, help, history_log};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Explorer,
    Editor,
}

/// Size of the terminal, as last reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u16,
    pub height: u16,
}

impl Viewport {
    /**
     * Rows around the editor pane: the tab line, the status line and the
     * command line.
     */
    const CHROME_ROWS: u16 = 3;
    /**
     * Narrowest terminal that shows the explorer by itself. Narrower ones
     * give its columns to the buffer, whose lines would be cut otherwise.
     */
    const EXPLORER_MIN_WIDTH: u16 = 100;

    pub fn editor_rows(self) -> usize {
        usize::from(self.height.saturating_sub(Self::CHROME_ROWS))
    }

    fn has_room_for_explorer(self) -> bool {
        self.width >= Self::EXPLORER_MIN_WIDTH
    }
}

/**
 * The size assumed before the terminal reports its own: a common one, with
 * room for the explorer.
 */
impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 120,
            height: 30,
        }
    }
}

impl App {
    /**
     * Takes the new size of the terminal, keeping the scrolled buffers
     * within their content, and the explorer as it rests at that size.
     */
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = Viewport { width, height };
        self.show_sidebar(self.resting_sidebar());
        self.history_scroll = self.history_scroll.min(self.last_scroll(Buffer::History));
        self.help_scroll = self.help_scroll.min(self.last_scroll(Buffer::Help));
    }

    /**
     * The scroll of `buffer` that shows its last line at the bottom of the
     * editor pane, zero for buffers that do not scroll.
     */
    pub(super) fn last_scroll(&self, buffer: Buffer) -> usize {
        let lines = match buffer {
            Buffer::History => history_log::lines(self.history.records().len()).len(),
            Buffer::Help => help::LINES.len(),
            _ => 0,
        };
        lines.saturating_sub(self.viewport.editor_rows())
    }

    /**
     * Shows or hides the explorer as the user asks, whatever the width of
     * the terminal from then on.
     */
    pub(super) fn set_sidebar(&mut self, visible: bool) {
        self.sidebar_choice = Some(visible);
        self.show_sidebar(visible);
    }

    fn show_sidebar(&mut self, visible: bool) {
        self.sidebar = visible;
        if !visible {
            self.focus = Focus::Editor;
        }
    }

    /**
     * Whether the explorer is shown while the editor has the focus: as the
     * user chose, or when the terminal has room for it.
     */
    fn resting_sidebar(&self) -> bool {
        self.sidebar_choice
            .unwrap_or_else(|| self.viewport.has_room_for_explorer())
    }

    /// Focuses the explorer, showing it while it has the focus.
    pub(super) fn focus_explorer(&mut self) {
        self.sidebar = true;
        self.focus = Focus::Explorer;
    }

    /// Focuses the editor, the explorer going back to how it rests.
    pub(super) fn focus_editor(&mut self) {
        self.focus = Focus::Editor;
        self.sidebar = self.resting_sidebar();
    }

    pub(super) fn focus_explorer_if_shown(&mut self) {
        self.focus = if self.sidebar {
            Focus::Explorer
        } else {
            Focus::Editor
        };
    }

    pub(super) fn toggle_focus(&mut self) {
        match self.focus {
            Focus::Explorer => self.focus_editor(),
            Focus::Editor => self.focus_explorer(),
        }
    }
}
