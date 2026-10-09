/*!
 * The mouse: a click does what the keys of the thing clicked do, and the
 * wheel scrolls. What lies where is what the screen showed last, as the
 * interface reports it in [`Hits`].
 */

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use super::{App, Buffer, Focus, form::Cursor, practice, race, settings};

/// Lines the wheel scrolls a document by.
const WHEEL_LINES: usize = 3;

/// What can be clicked on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// An entry of the explorer.
    Entry(Buffer),
    Tab(Buffer),
    /// The `index`th line of values of the form on screen.
    FormLine(usize),
    /// The `index`th match of the command palette.
    PaletteEntry(usize),
    /// The rest of the command palette, which a click leaves open.
    Overlay,
    /**
     * A key shown on screen, such as `r  new text` under the results,
     * pressed `times` times.
     */
    Key {
        code: KeyCode,
        times: u8,
    },
    /// The editor pane, which the wheel scrolls.
    Editor,
    /// The mode in the status line, which opens the command palette.
    Mode,
}

/**
 * Where each target lies on screen, as last drawn. A region added later
 * lies over the ones added before it.
 */
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hits {
    regions: Vec<(Rect, Target)>,
}

impl Hits {
    pub fn add(&mut self, area: Rect, target: Target) {
        if area.area() > 0 {
            self.regions.push((area, target));
        }
    }

    /// The target at `column` and `row`, the topmost one.
    pub fn at(&self, column: u16, row: u16) -> Option<Target> {
        let position = Position::new(column, row);
        self.regions
            .iter()
            .rev()
            .find(|(area, _)| area.contains(position))
            .map(|(_, target)| *target)
    }
}

impl App {
    /**
     * Handles a mouse event on the screen mapped by `hits`. Returns
     * whether it changed anything: moves and releases do not, so that
     * moving the mouse never redraws.
     */
    pub fn handle_mouse(&mut self, mouse: MouseEvent, hits: &Hits, now: Instant) -> bool {
        if !self.config.mouse {
            return false;
        }
        let wheel = match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => None,
            MouseEventKind::ScrollDown => Some(true),
            MouseEventKind::ScrollUp => Some(false),
            _ => return false,
        };
        self.begin_event();
        self.last_input = Some(now);
        if self.is_quiet(now) {
            return true;
        }
        let target = hits.at(mouse.column, mouse.row);
        match wheel {
            Some(down) => self.wheel(target, down, now),
            None => self.click(target, now),
        }
        true
    }

    fn click(&mut self, target: Option<Target>, now: Instant) {
        if self.palette.is_some() {
            match target {
                Some(Target::PaletteEntry(index)) => self.run_palette_entry(Some(index)),
                Some(Target::Overlay) => {}
                _ => self.palette = None,
            }
            return;
        }
        if self.is_typing() {
            return;
        }
        self.dismiss_message();
        self.prompt = None;
        match target {
            Some(Target::Entry(buffer)) => {
                if buffer != self.buffer {
                    self.cancel_edit();
                }
                self.buffer = buffer;
                self.focus_editor();
            }
            Some(Target::Tab(buffer)) => self.open(buffer),
            Some(Target::FormLine(index)) => self.click_form_line(index, now),
            Some(Target::Key { code, times }) => {
                for _ in 0..times {
                    self.dispatch_key(KeyEvent::from(code), now);
                }
            }
            Some(Target::Mode) => self.open_palette(),
            Some(Target::Editor) => {
                if self.focus == Focus::Explorer {
                    self.focus_editor();
                }
            }
            Some(Target::PaletteEntry(_) | Target::Overlay) | None => {}
        }
    }

    /**
     * Selects the line `index` of the form on screen, and acts on it when
     * it is selected already, or when it is an action such as `▶ start`.
     */
    fn click_form_line(&mut self, index: usize, now: Instant) {
        let Some((len, action)) = self.form_line(index) else {
            return;
        };
        self.focus_editor();
        let Some(cursor) = self.form_cursor_mut() else {
            return;
        };
        let selected = cursor.index(len) == index;
        cursor.select(index, len);
        if selected || action {
            self.dispatch_key(KeyEvent::from(KeyCode::Enter), now);
        }
    }

    /**
     * The number of lines of the form on screen, and whether its line
     * `index` is an action, when there is such a line.
     */
    fn form_line(&self, index: usize) -> Option<(usize, bool)> {
        match self.buffer {
            Buffer::Practice => {
                let fields = practice::fields(&self.config.practice);
                let field = *fields.get(index)?;
                Some((fields.len(), field == practice::Field::Start))
            }
            Buffer::Race => {
                let fields = race::fields(&self.config.race);
                let field = *fields.get(index)?;
                let action = matches!(field, race::Field::Join | race::Field::Create);
                Some((fields.len(), action))
            }
            Buffer::Settings => {
                settings::FIELDS.get(index)?;
                Some((settings::FIELDS.len(), false))
            }
            _ => None,
        }
    }

    fn form_cursor_mut(&mut self) -> Option<&mut Cursor> {
        match self.buffer {
            Buffer::Practice => Some(&mut self.practice_cursor),
            Buffer::Race => Some(&mut self.race_cursor),
            Buffer::Settings => Some(&mut self.settings_cursor),
            _ => None,
        }
    }

    fn wheel(&mut self, target: Option<Target>, down: bool, now: Instant) {
        if self.palette.is_some() {
            let count = self.palette_matches().len();
            if let Some(palette) = &mut self.palette {
                palette.scroll(down, count);
            }
            return;
        }
        if self.is_typing() || self.editing.is_some() || target != Some(Target::Editor) {
            return;
        }
        let key = KeyEvent::from(if down { KeyCode::Down } else { KeyCode::Up });
        let lines = match self.buffer {
            Buffer::History | Buffer::Help => WHEEL_LINES,
            _ => 1,
        };
        let focus = self.focus;
        self.focus = Focus::Editor;
        for _ in 0..lines {
            self.context_key(key, now);
        }
        self.focus = focus;
    }
}

#[cfg(test)]
mod tests;
