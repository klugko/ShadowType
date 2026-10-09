/*!
 * The command palette: every action of the application, listed and found
 * by typing a few letters of it, as in a code editor. It opens with Ctrl+P
 * or F1 from anywhere.
 */

mod matching;

use code_racer_engine::{CodeLanguage, Language};
use crossterm::event::{KeyCode, KeyEvent};

use super::{
    Activity, App, Buffer, TextField,
    command::{Command, Page, Setting},
    input::{Edit, TextInput, control_letter},
    practice::Plan,
};
use crate::config::{Icons, Look, Practice, Theme};
pub use matching::{Match, matches};

/// Longest query, in characters.
const MAX_QUERY: usize = 60;

/// The palette while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPalette {
    pub query: TextInput,
    /// Index of the selected entry among those that match the query.
    selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Run(Command),
    Restart,
    /// Opens the `:` line with this already typed.
    Prompt(&'static str),
    /// Opens `race.toml` on the room line, ready to type a code.
    JoinRoom,
    /// Opens `config.toml` on the name line, ready to type it.
    Rename,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    /// The key or command that does the same, shown on the right.
    pub shortcut: String,
    pub action: Action,
}

impl CommandPalette {
    fn new() -> Self {
        Self {
            query: TextInput::new("", MAX_QUERY),
            selected: 0,
        }
    }

    pub fn selected(&self, count: usize) -> usize {
        self.selected.min(count.saturating_sub(1))
    }

    pub fn select(&mut self, index: usize, count: usize) {
        self.selected = index.min(count.saturating_sub(1));
    }

    /**
     * Moves the selection one entry, without wrapping around, as the
     * wheel does.
     */
    pub fn scroll(&mut self, down: bool, count: usize) {
        let selected = self.selected(count);
        let selected = if down {
            selected + 1
        } else {
            selected.saturating_sub(1)
        };
        self.select(selected, count);
    }

    fn step(&mut self, down: bool, count: usize) {
        if count == 0 {
            return;
        }
        let current = self.selected(count);
        self.selected = if down {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
    }
}

/// Every entry, as they read with the settings of `app`.
pub fn entries(app: &App) -> Vec<Entry> {
    let mut entries = session_entries(app);
    entries.extend(text_entries(app));
    entries.extend(interface_entries(app));
    entries.extend(navigation_entries());
    entries
}

fn session_entries(app: &App) -> Vec<Entry> {
    let label = Plan::from_practice(&app.config.practice).label();
    let mut entries = vec![run(format!("Start a session: {label}"), "s", Command::Solo)];
    if matches!(app.activity, Some(Activity::Solo(_))) {
        entries.push(entry("Restart with a new text", "Ctrl+R", Action::Restart));
    }
    entries.extend(Practice::WORD_COUNT_PRESETS.map(|count| {
        run(
            format!("Words: {count} words"),
            format!(":words {count}"),
            Command::Words(Some(count)),
        )
    }));
    entries.extend(Practice::DURATION_PRESETS.map(|seconds| {
        run(
            format!("Time: {seconds} seconds"),
            format!(":time {seconds}"),
            Command::Time(Some(seconds)),
        )
    }));
    entries.push(run(
        "Quote: a passage of literature",
        ":quote",
        Command::Quote,
    ));
    entries.extend(CodeLanguage::ALL.map(|language| {
        run(
            format!("Code: {} snippet", language_title(language)),
            format!(":code {}", language.name()),
            Command::Code(Some(language)),
        )
    }));
    entries.push(entry(
        "Practise on a file of yours…",
        ":e PATH",
        Action::Prompt("e "),
    ));
    entries
}

fn text_entries(app: &App) -> Vec<Entry> {
    let practice = &app.config.practice;
    let looks = Look::ALL.map(|look| {
        let current = if look == app.config.look {
            " (current)"
        } else {
            ""
        };
        set(
            format!("Look: prose as {}{current}", look.describe()),
            format!(":set look={look}"),
            Setting::Look(look),
        )
    });
    let languages = Language::ALL.map(|language| {
        run(
            format!("Language: {}", title_case(&language.to_string())),
            format!(":lang {language}"),
            Command::Language(language),
        )
    });
    looks
        .into_iter()
        .chain(languages)
        .chain([
            toggle(
                "Punctuation",
                practice.punctuation,
                "punctuation",
                Setting::Punctuation,
            ),
            toggle("Numbers", practice.numbers, "numbers", Setting::Numbers),
        ])
        .collect()
}

fn interface_entries(app: &App) -> Vec<Entry> {
    let config = &app.config;
    let themes = Theme::ALL.map(|theme| {
        let name = match theme {
            Theme::VsCode => "VS Code Dark+".to_owned(),
            theme => title_case(theme.name()),
        };
        set(
            format!("Theme: {name}"),
            format!(":set theme={theme}"),
            Setting::Theme(theme),
        )
    });
    let icons = Icons::ALL.map(|icons| {
        let what = match icons {
            Icons::Unicode => "symbols of every font",
            Icons::Nerd => "Nerd Font",
            Icons::None => "none",
        };
        set(
            format!("File icons: {what}"),
            format!(":set icons={icons}"),
            Setting::Icons(icons),
        )
    });
    let discreet = Entry {
        shortcut: "F12".to_owned(),
        ..toggle(
            "Discreet mode",
            config.discreet,
            "discreet",
            Setting::Discreet,
        )
    };
    themes
        .into_iter()
        .chain(icons)
        .chain([
            switch("Explorer", app.sidebar, "Ctrl+B", Setting::Sidebar),
            switch("Mascot", config.mascot, ":set mascot", Setting::Mascot),
            toggle(
                "Animations",
                config.animations,
                "animations",
                Setting::Animations,
            ),
            toggle("Cursor trail", config.trail, "trail", Setting::Trail),
            toggle("Mouse", config.mouse, "mouse", Setting::Mouse),
            discreet,
        ])
        .collect()
}

fn navigation_entries() -> Vec<Entry> {
    vec![
        run("Race: create a room", "c", Command::Create),
        entry("Race: join a room…", ":join CODE", Action::JoinRoom),
        entry("Set your name…", ":set username=", Action::Rename),
        run(
            "Open practice.toml",
            ":practice",
            Command::Open(Page::Practice),
        ),
        run("Open race.toml", "m", Command::Open(Page::Race)),
        run("Open history.log", ":history", Command::Open(Page::History)),
        run("Open config.toml", ":config", Command::Open(Page::Settings)),
        run("Open help.md", "?", Command::Open(Page::Help)),
        run("Quit", ":q", Command::Quit),
    ]
}

fn entry(title: impl Into<String>, shortcut: impl Into<String>, action: Action) -> Entry {
    Entry {
        title: title.into(),
        shortcut: shortcut.into(),
        action,
    }
}

fn run(title: impl Into<String>, shortcut: impl Into<String>, command: Command) -> Entry {
    entry(title, shortcut, Action::Run(command))
}

fn set(title: impl Into<String>, shortcut: impl Into<String>, setting: Setting) -> Entry {
    run(title, shortcut, Command::Set(setting))
}

/// An entry turning an option on or off, the other way from `on`.
fn toggle(name: &str, on: bool, option: &str, setting: fn(bool) -> Setting) -> Entry {
    let (verb, prefix) = if on { ("off", "no") } else { ("on", "") };
    set(
        format!("{name}: turn {verb}"),
        format!(":set {prefix}{option}"),
        setting(!on),
    )
}

/// An entry showing or hiding something, the other way from `shown`.
fn switch(name: &str, shown: bool, shortcut: &str, setting: fn(bool) -> Setting) -> Entry {
    let verb = if shown { "hide" } else { "show" };
    set(format!("{name}: {verb}"), shortcut, setting(!shown))
}

/// The name of a programming language as its users write it.
fn language_title(language: CodeLanguage) -> &'static str {
    match language {
        CodeLanguage::Rust => "Rust",
        CodeLanguage::Python => "Python",
        CodeLanguage::TypeScript => "TypeScript",
        CodeLanguage::JavaScript => "JavaScript",
        CodeLanguage::Sql => "SQL",
    }
}

fn title_case(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

impl App {
    pub(super) fn open_palette(&mut self) {
        self.prompt = None;
        self.palette = Some(CommandPalette::new());
    }

    /// The entries of the open palette that match its query, best first.
    pub fn palette_matches(&self) -> Vec<Match> {
        self.palette
            .as_ref()
            .map(|palette| matches(entries(self), palette.query.value()))
            .unwrap_or_default()
    }

    pub(super) fn palette_key(&mut self, key: KeyEvent) {
        let count = self.palette_matches().len();
        let Some(palette) = &mut self.palette else {
            return;
        };
        match (control_letter(key), key.code) {
            (Some('n' | 'j'), _) | (_, KeyCode::Down | KeyCode::Tab) => palette.step(true, count),
            (Some('p' | 'k'), _) | (_, KeyCode::Up | KeyCode::BackTab) => {
                palette.step(false, count);
            }
            (_, KeyCode::PageDown) => palette.select(palette.selected(count) + 5, count),
            (_, KeyCode::PageUp) => {
                let selected = palette.selected(count).saturating_sub(5);
                palette.select(selected, count);
            }
            _ => match palette.query.handle_key(key) {
                Edit::Submitted => self.run_palette_entry(None),
                Edit::Cancelled => self.palette = None,
                Edit::Changed => palette.selected = 0,
                Edit::Ignored => {}
            },
        }
    }

    /**
     * Runs the entry `index` of the matches, or the selected one, and
     * closes the palette.
     */
    pub(super) fn run_palette_entry(&mut self, index: Option<usize>) {
        let matches = self.palette_matches();
        let Some(palette) = self.palette.take() else {
            return;
        };
        let index = index.unwrap_or_else(|| palette.selected(matches.len()));
        let Some(found) = matches.into_iter().nth(index) else {
            return;
        };
        match found.entry.action {
            Action::Run(command) => self.run_command(command),
            Action::Restart => self.restart_solo(),
            Action::Prompt(text) => {
                let mut prompt = super::Prompt::new();
                prompt.input.insert_str(text);
                self.prompt = Some(prompt);
            }
            Action::JoinRoom => {
                self.open(Buffer::Race);
                self.race_cursor.first();
                self.begin_edit(TextField::RoomCode);
            }
            Action::Rename => {
                self.open(Buffer::Settings);
                self.settings_cursor.first();
                self.begin_edit(TextField::Username);
            }
        }
    }
}

#[cfg(test)]
mod tests;
