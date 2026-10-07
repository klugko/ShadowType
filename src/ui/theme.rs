//! Colour palettes. `editor` uses true colours, `dark` the 16 ANSI colours of
//! the terminal on a black background, `mono` no colour at all.

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

const EDITOR_HIGHLIGHT: Color = Color::Rgb(36, 40, 50);

const EDITOR: Palette = Palette {
    background: Color::Rgb(26, 29, 36),
    panel: Color::Rgb(21, 24, 30),
    highlight: EDITOR_HIGHLIGHT,
    cursorline: Style::new().bg(EDITOR_HIGHLIGHT),
    border: Color::Rgb(44, 49, 60),
    text: Color::Rgb(171, 178, 191),
    strong: Color::Rgb(214, 219, 228),
    muted: Color::Rgb(92, 99, 112),
    faint: Color::Rgb(70, 76, 89),
    accent: Color::Rgb(97, 166, 230),
    success: Color::Rgb(140, 190, 120),
    warning: Color::Rgb(222, 186, 112),
    error: Color::Rgb(226, 108, 117),
    insert: Color::Rgb(140, 190, 120),
    command: Color::Rgb(222, 186, 112),
    keyword: Color::Rgb(194, 130, 220),
    kind: Color::Rgb(222, 186, 112),
    function: Color::Rgb(97, 166, 230),
    string: Color::Rgb(140, 190, 120),
    number: Color::Rgb(214, 150, 98),
    comment: Color::Rgb(106, 114, 128),
    punctuation: Color::Rgb(150, 158, 172),
    on_accent: Color::Rgb(21, 24, 30),
    mono: false,
};

const DARK: Palette = Palette {
    background: Color::Black,
    panel: Color::Black,
    highlight: Color::DarkGray,
    cursorline: Style::new().add_modifier(Modifier::BOLD),
    border: Color::DarkGray,
    text: Color::Gray,
    strong: Color::White,
    muted: Color::DarkGray,
    faint: Color::DarkGray,
    accent: Color::Blue,
    success: Color::Green,
    warning: Color::Yellow,
    error: Color::Red,
    insert: Color::Green,
    command: Color::Yellow,
    keyword: Color::Magenta,
    kind: Color::Yellow,
    function: Color::Blue,
    string: Color::Green,
    number: Color::Cyan,
    comment: Color::DarkGray,
    punctuation: Color::Gray,
    on_accent: Color::Black,
    mono: false,
};

const MONO: Palette = Palette {
    background: Color::Reset,
    panel: Color::Reset,
    highlight: Color::Reset,
    cursorline: Style::new().add_modifier(Modifier::BOLD),
    border: Color::Reset,
    text: Color::Reset,
    strong: Color::Reset,
    muted: Color::DarkGray,
    faint: Color::DarkGray,
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
    comment: Color::DarkGray,
    punctuation: Color::Reset,
    on_accent: Color::Reset,
    mono: true,
};
