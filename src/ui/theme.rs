//! Colour palettes. `editor` uses true colours, `dark` the 16 ANSI colours of
//! the terminal on a black background, `mono` no colour at all: it shows
//! every state with bold, dim, underlined or reversed text instead.

use ratatui::style::{Color, Modifier, Style};

use crate::{config::Theme, ui::syntax::Token};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub panel: Color,
    /// Background of the status line segments and of the selected explorer
    /// entry, which hold only strong text.
    pub highlight: Color,
    /// The line under the cursor. The ANSI palette makes it bold: its only
    /// grey is the one of the text still to type, which a grey background
    /// would hide.
    pub cursorline: Style,
    /// The typing cursor.
    pub cursor: Style,
    /// A character typed wrong. Never reversed like the cursor: next to it,
    /// a mistake would read as a wider cursor.
    pub mistake: Style,
    /// The text still to type, which must not read as typed text.
    pub pending: Style,
    /// The selected explorer entry while the explorer has the focus, unlike
    /// the bold entry it stays while the editor has it.
    pub selection: Style,
    pub border: Color,
    pub text: Color,
    pub strong: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub insert: Color,
    pub command: Color,
    pub keyword: Color,
    pub kind: Color,
    pub function: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub punctuation: Color,
    pub on_accent: Color,
    pub mono: bool,
}

impl Palette {
    pub fn of(theme: Theme) -> Self {
        match theme {
            Theme::Editor => EDITOR,
            Theme::Dark => DARK,
            Theme::Mono => MONO,
        }
    }

    pub fn base(&self) -> Style {
        Style::new().fg(self.text).bg(self.background)
    }

    pub fn fg(&self, color: Color) -> Style {
        Style::new().fg(color)
    }

    /// A pill such as the mode indicator of the status line.
    pub fn badge(&self, color: Color) -> Style {
        if self.mono {
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::new()
                .fg(self.on_accent)
                .bg(color)
                .add_modifier(Modifier::BOLD)
        }
    }

    pub fn syntax(&self, token: Token) -> Style {
        let color = match token {
            Token::Keyword => self.keyword,
            Token::Type => self.kind,
            Token::Function | Token::Macro => self.function,
            Token::String => self.string,
            Token::Number => self.number,
            Token::Comment => self.comment,
            Token::Punctuation => self.punctuation,
            Token::Plain => self.strong,
        };
        let style = Style::new().fg(color);
        if self.mono && token == Token::Keyword {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }
}

/// The typing cursor of a colour theme: a block of its accent colour.
const fn block(accent: Color, on_accent: Color) -> Style {
    Style::new().fg(on_accent).bg(accent)
}

/// A mistake in a colour theme: in its error colour, underlined.
const fn underlined(error: Color) -> Style {
    Style::new()
        .fg(error)
        .underline_color(error)
        .add_modifier(Modifier::UNDERLINED)
}

/// A selection in a colour theme: bold strong text on its highlight.
const fn highlighted(highlight: Color, strong: Color) -> Style {
    Style::new()
        .bg(highlight)
        .fg(strong)
        .add_modifier(Modifier::BOLD)
}

const EDITOR_HIGHLIGHT: Color = Color::Rgb(36, 40, 50);
const EDITOR_STRONG: Color = Color::Rgb(214, 219, 228);
const EDITOR_MUTED: Color = Color::Rgb(92, 99, 112);
/// Text still to type: dimmer than typed text, yet readable on the cursor line.
const EDITOR_PENDING: Color = Color::Rgb(138, 146, 160);
const EDITOR_ACCENT: Color = Color::Rgb(97, 166, 230);
const EDITOR_ERROR: Color = Color::Rgb(226, 108, 117);
const EDITOR_ON_ACCENT: Color = Color::Rgb(21, 24, 30);

const EDITOR: Palette = Palette {
    background: Color::Rgb(26, 29, 36),
    panel: Color::Rgb(21, 24, 30),
    highlight: EDITOR_HIGHLIGHT,
    cursorline: Style::new().bg(EDITOR_HIGHLIGHT),
    cursor: block(EDITOR_ACCENT, EDITOR_ON_ACCENT),
    mistake: underlined(EDITOR_ERROR),
    pending: Style::new().fg(EDITOR_PENDING),
    selection: highlighted(EDITOR_HIGHLIGHT, EDITOR_STRONG),
    border: Color::Rgb(44, 49, 60),
    text: Color::Rgb(171, 178, 191),
    strong: EDITOR_STRONG,
    muted: EDITOR_MUTED,
    faint: Color::Rgb(70, 76, 89),
    accent: EDITOR_ACCENT,
    success: Color::Rgb(140, 190, 120),
    warning: Color::Rgb(222, 186, 112),
    error: EDITOR_ERROR,
    insert: Color::Rgb(140, 190, 120),
    command: Color::Rgb(222, 186, 112),
    keyword: Color::Rgb(194, 130, 220),
    kind: Color::Rgb(222, 186, 112),
    function: Color::Rgb(97, 166, 230),
    string: Color::Rgb(140, 190, 120),
    number: Color::Rgb(214, 150, 98),
    comment: Color::Rgb(106, 114, 128),
    punctuation: Color::Rgb(150, 158, 172),
    on_accent: EDITOR_ON_ACCENT,
    mono: false,
};

const DARK_HIGHLIGHT: Color = Color::DarkGray;
const DARK_STRONG: Color = Color::White;
const DARK_MUTED: Color = Color::DarkGray;
const DARK_ACCENT: Color = Color::Blue;
const DARK_ERROR: Color = Color::Red;
const DARK_ON_ACCENT: Color = Color::Black;

const DARK: Palette = Palette {
    background: Color::Black,
    panel: Color::Black,
    highlight: DARK_HIGHLIGHT,
    cursorline: Style::new().add_modifier(Modifier::BOLD),
    cursor: block(DARK_ACCENT, DARK_ON_ACCENT),
    mistake: underlined(DARK_ERROR),
    pending: Style::new().fg(DARK_MUTED),
    selection: highlighted(DARK_HIGHLIGHT, DARK_STRONG),
    border: Color::DarkGray,
    text: Color::Gray,
    strong: DARK_STRONG,
    muted: DARK_MUTED,
    faint: Color::DarkGray,
    accent: DARK_ACCENT,
    success: Color::Green,
    warning: Color::Yellow,
    error: DARK_ERROR,
    insert: Color::Green,
    command: Color::Yellow,
    keyword: Color::Magenta,
    kind: Color::Yellow,
    function: Color::Blue,
    string: Color::Green,
    number: Color::Cyan,
    comment: Color::DarkGray,
    punctuation: Color::Gray,
    on_accent: DARK_ON_ACCENT,
    mono: false,
};

/// The text still to type in `mono`: dim, and never bold, as on the bold
/// cursor line some terminals let bold win over dim.
const MONO_PENDING: Style = Style::new()
    .add_modifier(Modifier::DIM)
    .remove_modifier(Modifier::BOLD);

const MONO: Palette = Palette {
    background: Color::Reset,
    panel: Color::Reset,
    highlight: Color::Reset,
    cursorline: Style::new().add_modifier(Modifier::BOLD),
    cursor: Style::new().add_modifier(Modifier::REVERSED),
    mistake: Style::new().add_modifier(Modifier::UNDERLINED.union(Modifier::BOLD)),
    pending: MONO_PENDING,
    selection: Style::new().add_modifier(Modifier::REVERSED.union(Modifier::BOLD)),
    border: Color::Reset,
    text: Color::Reset,
    strong: Color::Reset,
    muted: Color::Reset,
    faint: Color::Reset,
    accent: Color::Reset,
    success: Color::Reset,
    warning: Color::Reset,
    error: Color::Reset,
    insert: Color::Reset,
    command: Color::Reset,
    keyword: Color::Reset,
    kind: Color::Reset,
    function: Color::Reset,
    string: Color::Reset,
    number: Color::Reset,
    comment: Color::Reset,
    punctuation: Color::Reset,
    on_accent: Color::Reset,
    mono: true,
};
