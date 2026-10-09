use code_racer_engine::{CodeLanguage, Language};

use super::{Action, Entry};
use crate::{
    app::{
        Activity, App,
        command::{Command, Page, Setting},
        practice::Plan,
    },
    config::{Icons, Look, Practice, Theme},
};

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

pub(super) fn entry(
    title: impl Into<String>,
    shortcut: impl Into<String>,
    action: Action,
) -> Entry {
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
pub(super) fn toggle(name: &str, on: bool, option: &str, setting: fn(bool) -> Setting) -> Entry {
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
