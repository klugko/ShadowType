use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Color scheme of the interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// The default code editor look.
    #[default]
    Editor,
    /// A dark background whatever the terminal's own.
    Dark,
    /// Monochrome, for terminals without colors.
    Mono,
    /// The colours of VS Code's default dark theme, its blue status bar
    /// included.
    #[value(name = "vscode")]
    VsCode,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::Editor, Self::Dark, Self::Mono, Self::VsCode];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Dark => "dark",
            Self::Mono => "mono",
            Self::VsCode => "vscode",
        }
    }
}

/// The icons in front of file names, in the explorer and the tabs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Icons {
    /// Symbols every terminal font has, coloured by file type.
    #[default]
    Unicode,
    /// The icons of a Nerd Font, as in a code editor's file icon theme.
    Nerd,
    /// No icons.
    None,
}

impl Icons {
    pub const ALL: [Self; 3] = [Self::Unicode, Self::Nerd, Self::None];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Unicode => "unicode",
            Self::Nerd => "nerd",
            Self::None => "none",
        }
    }
}

/**
 * What a prose text looks like on screen: the kind of file it is typed in,
 * so that a glance at the screen shows someone writing, not practising.
 */
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Look {
    /// Markdown notes: the text as it comes.
    #[default]
    Notes,
    /// A markdown checklist, ticked off row by row.
    Todo,
    /// A git commit message, git's template under it.
    Commit,
    /// The documentation comment of a function, in the code language.
    Docs,
    /// A log, each row stamped with the time it was typed.
    Log,
    /// An email draft.
    Mail,
    /// Another of the others for every text.
    Shuffle,
}

impl Look {
    pub const ALL: [Self; 7] = [
        Self::Notes,
        Self::Todo,
        Self::Commit,
        Self::Docs,
        Self::Log,
        Self::Mail,
        Self::Shuffle,
    ];
    /// The looks a text can have on screen, every one but [`Look::Shuffle`].
    pub const DISGUISES: [Self; 6] = [
        Self::Notes,
        Self::Todo,
        Self::Commit,
        Self::Docs,
        Self::Log,
        Self::Mail,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Notes => "notes",
            Self::Todo => "todo",
            Self::Commit => "commit",
            Self::Docs => "docs",
            Self::Log => "log",
            Self::Mail => "mail",
            Self::Shuffle => "shuffle",
        }
    }

    /// What the look makes texts look like, for messages and hints.
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Notes => "markdown notes",
            Self::Todo => "a todo list",
            Self::Commit => "a commit message",
            Self::Docs => "a doc comment",
            Self::Log => "a log file",
            Self::Mail => "an email draft",
            Self::Shuffle => "a different file every time",
        }
    }

    /**
     * The look a shuffle gives the text after one that looked like
     * `previous`: another of the [`Look::DISGUISES`], picked with `seed`.
     */
    pub fn shuffled_after(previous: Self, seed: u64) -> Self {
        let others: Vec<Self> = Self::DISGUISES
            .into_iter()
            .filter(|look| *look != previous)
            .collect();
        let index = seed % u64::try_from(others.len()).unwrap_or(1);
        others[usize::try_from(index).unwrap_or(0)]
    }
}
