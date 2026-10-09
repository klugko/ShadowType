//! The mouse: a click does what the keys of the thing clicked do, and the
//! wheel scrolls. What lies where is what the screen showed last, as the
//! interface reports it in [`Hits`].

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
    /// A key shown on screen, such as `r  new text` under the results,
    /// pressed `times` times.
    Key {
        code: KeyCode,
        times: u8,
    },
    /// The editor pane, which the wheel scrolls.
    Editor,
    /// The mode in the status line, which opens the command palette.
    Mode,
}

/// Where each target lies on screen, as last drawn. A region added later
/// lies over the ones added before it.
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
    /// Handles a mouse event on the screen mapped by `hits`. Returns
    /// whether it changed anything: moves and releases do not, so that
    /// moving the mouse never redraws.
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

    /// Selects the line `index` of the form on screen, and acts on it when
    /// it is selected already, or when it is an action such as `▶ start`.
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

    /// The number of lines of the form on screen, and whether its line
    /// `index` is an action, when there is such a line.
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
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;
    use crate::{
        app::{Overrides, test_support::type_next},
        cli::Launch,
        config::{Config, Mode},
        history::History,
    };

    fn app() -> App {
        let config = Config {
            username: "ada".to_owned(),
            ..Config::default()
        };
        App::new(
            config,
            &Overrides::default(),
            None,
            History::in_memory(),
            Vec::new(),
            Launch::Home,
        )
    }

    fn click(app: &mut App, hits: &Hits, (column, row): (u16, u16)) -> bool {
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(event, hits, Instant::now())
    }

    fn hits(targets: &[Target]) -> Hits {
        let mut hits = Hits::default();
        for (row, target) in (0..).zip(targets) {
            hits.add(Rect::new(0, row, 10, 1), *target);
        }
        hits
    }

    #[test]
    fn the_topmost_region_wins() {
        let mut hits = Hits::default();
        hits.add(Rect::new(0, 0, 80, 20), Target::Editor);
        hits.add(Rect::new(2, 3, 5, 1), Target::Mode);
        assert_eq!(hits.at(3, 3), Some(Target::Mode));
        assert_eq!(hits.at(30, 3), Some(Target::Editor));
        assert_eq!(hits.at(90, 3), None);
    }

    #[test]
    fn clicking_an_explorer_entry_opens_it() {
        let mut app = app();
        let hits = hits(&[Target::Entry(Buffer::History)]);
        assert!(click(&mut app, &hits, (1, 0)));
        assert_eq!((app.buffer, app.focus), (Buffer::History, Focus::Editor));
    }

    #[test]
    fn a_form_line_is_selected_then_changed() {
        let mut app = app();
        app.buffer = Buffer::Practice;
        let words = 2;
        let hits = hits(&[Target::FormLine(words)]);
        click(&mut app, &hits, (1, 0));
        assert_eq!(app.practice_cursor.index(6), words);
        assert_eq!(app.config.practice.word_count, 50, "selected first");
        click(&mut app, &hits, (1, 0));
        assert_eq!(app.config.practice.word_count, 100, "then changed");
        assert_eq!(app.config.practice.mode, Mode::Words);
    }

    #[test]
    fn an_action_line_acts_at_once() {
        let mut app = app();
        app.buffer = Buffer::Practice;
        let start = practice::fields(&app.config.practice).len() - 1;
        let hits = hits(&[Target::FormLine(start)]);
        click(&mut app, &hits, (1, 0));
        assert!(app.solo().is_some(), "the session started");
        assert_eq!(app.buffer, Buffer::Session);
    }

    #[test]
    fn clicks_never_leave_the_text_being_typed() {
        let mut app = app();
        app.start_practice();
        type_next(&mut app, Instant::now());
        let hits = hits(&[Target::Entry(Buffer::Help)]);
        click(&mut app, &hits, (1, 0));
        assert!(app.is_typing());
    }

    #[test]
    fn moving_the_mouse_changes_nothing() {
        let mut app = app();
        let moved = MouseEvent {
            kind: MouseEventKind::Moved,
            column: 1,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!app.handle_mouse(moved, &Hits::default(), Instant::now()));
    }

    #[test]
    fn the_mouse_can_be_left_to_the_terminal() {
        let mut app = app();
        app.config.mouse = false;
        let hits = hits(&[Target::Entry(Buffer::History)]);
        assert!(!click(&mut app, &hits, (1, 0)));
        assert_eq!(app.buffer, Buffer::Practice);
    }

    #[test]
    fn the_wheel_scrolls_the_document() {
        let mut app = app();
        app.resize(80, 20);
        app.buffer = Buffer::Help;
        app.focus = Focus::Editor;
        let hits = hits(&[Target::Editor]);
        let wheel = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(wheel, &hits, Instant::now());
        assert_eq!(app.help_scroll, WHEEL_LINES);
    }

    #[test]
    fn a_shown_key_is_pressed_by_a_click() {
        let mut app = app();
        let hits = hits(&[Target::Key {
            code: KeyCode::Char('?'),
            times: 1,
        }]);
        click(&mut app, &hits, (1, 0));
        assert_eq!(app.buffer, Buffer::Help);
    }

    #[test]
    fn clicking_a_palette_entry_runs_it() {
        let mut app = app();
        app.open_palette();
        let index = app
            .palette_matches()
            .iter()
            .position(|found| found.entry.title == "Open help.md")
            .expect("the entry");
        let hits = hits(&[Target::PaletteEntry(index)]);
        click(&mut app, &hits, (1, 0));
        assert!(app.palette.is_none());
        assert_eq!(app.buffer, Buffer::Help);
    }
}
