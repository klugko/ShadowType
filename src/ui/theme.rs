/*!
 * Colour palettes: `editor` and `vscode` (VS Code's Dark+) in true colours,
 * `dark` in the 16 ANSI colours on black, and `mono` without colour, telling
 * every state apart by bold, dim, underlined or reversed text instead.
 */

use ratatui::style::{Color, Modifier, Style};

use crate::{config::Theme, ui::syntax::Token};

#[derive(Debug, Clone, Copy)]
pub struct SyntaxColors {
    pub keyword: Color,
    pub kind: Color,
    pub function: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub punctuation: Color,
    pub plain: Color,
    /// By nesting depth, like an editor's bracket pair colouring.
    pub brackets: [Color; 3],
}

impl SyntaxColors {
    pub fn of(&self, token: Token) -> Color {
        match token {
            Token::Keyword => self.keyword,
            Token::Type => self.kind,
            Token::Function | Token::Macro => self.function,
            Token::String => self.string,
            Token::Number => self.number,
            Token::Comment => self.comment,
            Token::Punctuation => self.punctuation,
            Token::Plain => self.plain,
        }
    }

    pub fn bracket(&self, depth: usize) -> Color {
        self.brackets[depth % self.brackets.len()]
    }
}

/**
 * `from` blended into `to` by `amount`, from 0 (all `from`) to 1 (all
 * `to`), when both are true colours; `to` otherwise.
 */
pub fn blend(from: Color, to: Color, amount: f64) -> Color {
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (from, to) else {
        return to;
    };
    let amount = amount.clamp(0.0, 1.0);
    let channel = |from: u8, to: u8| {
        let value = f64::from(from) + (f64::from(to) - f64::from(from)) * amount;
        value.round().clamp(0.0, 255.0) as u8
    };
    Color::Rgb(channel(r1, r2), channel(g1, g2), channel(b1, b2))
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub panel: Color,
    /**
     * Background of the status line segments and of the selected explorer
     * entry, which hold only strong text.
     */
    pub highlight: Color,
    /**
     * The line under the cursor. Bold in the ANSI palette, whose only grey is
     * the one of the text still to type, which a grey background would hide.
     */
    pub cursorline: Style,
    pub cursor: Style,
    /**
     * A character typed wrong. Never reversed like the cursor: next to it,
     * a mistake would read as a wider cursor.
     */
    pub mistake: Style,
    /// The text still to type, which must not read as typed text.
    pub pending: Style,
    /**
     * The selected explorer entry while the explorer has the focus, unlike
     * the bold entry it stays while the editor has it.
     */
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
    /// Code once typed, brighter than the code still to type.
    pub lit: SyntaxColors,
    /**
     * Code still to type, in the colours of its syntax dimmed; `None`
     * where only grey tells it apart, as on 16 colours.
     */
    pub ghost: Option<SyntaxColors>,
    /**
     * What freshly typed text glows with before it dries, where colours
     * can blend.
     */
    pub glow: Option<Color>,
    pub status: Style,
    /// The segments the status line sets apart, such as the name of the file.
    pub status_item: Style,
    /// The number of errors in the status line, once there are some.
    pub status_alert: Color,
    /// Whether the rows past the end of a buffer show `~`, as in Vim.
    pub tildes: bool,
    pub mono: bool,
}

impl Palette {
    pub fn of(theme: Theme) -> Self {
        match theme {
            Theme::Editor => EDITOR,
            Theme::Dark => DARK,
            Theme::Mono => MONO,
            Theme::VsCode => VSCODE,
        }
    }

    /// Whether colours blend, for what fades in and out: true colours only.
    pub fn blends(&self) -> bool {
        self.glow.is_some()
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

    /// Code once typed, `None` for a character outside any token.
    pub fn typed(&self, token: Option<Token>) -> Style {
        let Some(token) = token else {
            return self.fg(self.strong);
        };
        let style = Style::new().fg(self.lit.of(token));
        if self.mono && token == Token::Keyword {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }

    /// Code still to type, `None` for a character outside any token.
    pub fn ghost(&self, token: Option<Token>) -> Style {
        match (self.ghost, token) {
            (Some(ghost), Some(token)) => self.pending.fg(ghost.of(token)),
            _ => self.pending,
        }
    }

    pub fn bracket(&self, depth: usize, typed: bool) -> Style {
        match (typed, self.ghost) {
            _ if self.mono => self.typed(Some(Token::Punctuation)),
            (true, _) => Style::new().fg(self.lit.bracket(depth)),
            (false, Some(ghost)) => self.pending.fg(ghost.bracket(depth)),
            (false, None) => self.pending,
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

const fn block(accent: Color, on_accent: Color) -> Style {
    Style::new().fg(on_accent).bg(accent)
}

const fn underlined(error: Color) -> Style {
    Style::new()
        .fg(error)
        .underline_color(error)
        .add_modifier(Modifier::UNDERLINED)
}

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

const EDITOR_PANEL: Color = Color::Rgb(21, 24, 30);
const EDITOR_TEXT: Color = Color::Rgb(171, 178, 191);

const EDITOR: Palette = Palette {
    background: Color::Rgb(26, 29, 36),
    panel: EDITOR_PANEL,
    highlight: EDITOR_HIGHLIGHT,
    cursorline: Style::new().bg(EDITOR_HIGHLIGHT),
    cursor: block(EDITOR_ACCENT, EDITOR_ON_ACCENT),
    mistake: underlined(EDITOR_ERROR),
    pending: Style::new().fg(EDITOR_PENDING),
    selection: highlighted(EDITOR_HIGHLIGHT, EDITOR_STRONG),
    border: Color::Rgb(44, 49, 60),
    text: EDITOR_TEXT,
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
    lit: SyntaxColors {
        keyword: Color::Rgb(226, 197, 239),
        kind: Color::Rgb(230, 204, 148),
        function: Color::Rgb(176, 211, 243),
        string: Color::Rgb(185, 216, 173),
        number: Color::Rgb(234, 201, 174),
        comment: Color::Rgb(178, 183, 191),
        punctuation: Color::Rgb(203, 207, 214),
        plain: EDITOR_STRONG,
        brackets: [
            Color::Rgb(242, 204, 0),
            Color::Rgb(239, 192, 237),
            Color::Rgb(155, 214, 255),
        ],
    },
    ghost: Some(SyntaxColors {
        keyword: Color::Rgb(173, 128, 192),
        kind: Color::Rgb(171, 141, 78),
        function: Color::Rgb(100, 149, 194),
        string: Color::Rgb(116, 155, 101),
        number: Color::Rgb(180, 136, 99),
        comment: Color::Rgb(119, 124, 133),
        punctuation: Color::Rgb(139, 145, 154),
        plain: EDITOR_PENDING,
        brackets: [
            Color::Rgb(164, 145, 41),
            Color::Rgb(190, 120, 188),
            Color::Rgb(69, 151, 209),
        ],
    }),
    glow: Some(EDITOR_ACCENT),
    status: Style::new().bg(EDITOR_PANEL).fg(EDITOR_TEXT),
    status_item: Style::new().bg(EDITOR_HIGHLIGHT).fg(EDITOR_STRONG),
    status_alert: EDITOR_ERROR,
    tildes: true,
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
    lit: SyntaxColors {
        keyword: Color::Magenta,
        kind: Color::Yellow,
        function: Color::Blue,
        string: Color::Green,
        number: Color::Cyan,
        comment: Color::Gray,
        punctuation: Color::Gray,
        plain: DARK_STRONG,
        brackets: [Color::Yellow, Color::Magenta, Color::Cyan],
    },
    ghost: None,
    glow: None,
    status: Style::new().bg(Color::Black).fg(Color::Gray),
    status_item: Style::new().bg(DARK_HIGHLIGHT).fg(DARK_STRONG),
    status_alert: DARK_ERROR,
    tildes: true,
    mono: false,
};

const VSCODE_BACKGROUND: Color = Color::Rgb(30, 30, 30);
const VSCODE_CURRENT_LINE: Color = Color::Rgb(40, 40, 40);
const VSCODE_FOREGROUND: Color = Color::Rgb(212, 212, 212);
/// Text still to type: 4.6:1 on the current line, typed text 2.1:1 on it.
const VSCODE_PENDING: Color = Color::Rgb(144, 144, 144);
const VSCODE_BLUE: Color = Color::Rgb(0, 122, 204);
const VSCODE_WHITE: Color = Color::Rgb(255, 255, 255);
const VSCODE_ERROR: Color = Color::Rgb(241, 76, 76);

const VSCODE: Palette = Palette {
    background: VSCODE_BACKGROUND,
    panel: Color::Rgb(37, 37, 38),
    highlight: Color::Rgb(42, 45, 46),
    cursorline: Style::new().bg(VSCODE_CURRENT_LINE),
    cursor: block(VSCODE_BLUE, VSCODE_WHITE),
    mistake: underlined(VSCODE_ERROR),
    pending: Style::new().fg(VSCODE_PENDING),
    selection: Style::new()
        .bg(Color::Rgb(4, 57, 94))
        .fg(VSCODE_WHITE)
        .add_modifier(Modifier::BOLD),
    border: Color::Rgb(60, 60, 60),
    text: Color::Rgb(204, 204, 204),
    strong: VSCODE_FOREGROUND,
    muted: Color::Rgb(133, 133, 133),
    faint: Color::Rgb(110, 110, 110),
    accent: VSCODE_BLUE,
    success: Color::Rgb(137, 209, 133),
    warning: Color::Rgb(204, 167, 0),
    error: VSCODE_ERROR,
    insert: Color::Rgb(22, 130, 93),
    command: Color::Rgb(136, 23, 152),
    keyword: Color::Rgb(86, 156, 214),
    kind: Color::Rgb(78, 201, 176),
    function: Color::Rgb(156, 220, 254),
    string: Color::Rgb(206, 145, 120),
    number: Color::Rgb(181, 206, 168),
    comment: Color::Rgb(106, 153, 85),
    punctuation: VSCODE_FOREGROUND,
    on_accent: VSCODE_WHITE,
    lit: SyntaxColors {
        keyword: Color::Rgb(86, 156, 214),
        kind: Color::Rgb(78, 201, 176),
        function: Color::Rgb(220, 220, 170),
        string: Color::Rgb(206, 145, 120),
        number: Color::Rgb(181, 206, 168),
        comment: Color::Rgb(106, 153, 85),
        punctuation: VSCODE_FOREGROUND,
        plain: Color::Rgb(156, 220, 254),
        brackets: [
            Color::Rgb(255, 215, 0),
            Color::Rgb(218, 112, 214),
            Color::Rgb(23, 159, 255),
        ],
    },
    ghost: Some(SyntaxColors {
        keyword: Color::Rgb(66, 119, 164),
        kind: Color::Rgb(67, 147, 130),
        function: Color::Rgb(153, 153, 84),
        string: Color::Rgb(156, 102, 79),
        number: Color::Rgb(125, 157, 109),
        comment: Color::Rgb(94, 123, 82),
        punctuation: Color::Rgb(149, 149, 149),
        plain: Color::Rgb(110, 160, 190),
        brackets: [
            Color::Rgb(171, 149, 30),
            Color::Rgb(176, 73, 173),
            Color::Rgb(32, 120, 181),
        ],
    }),
    glow: Some(Color::Rgb(55, 148, 255)),
    status: Style::new().bg(VSCODE_BLUE).fg(VSCODE_WHITE),
    status_item: Style::new().bg(Color::Rgb(0, 95, 160)).fg(VSCODE_WHITE),
    status_alert: VSCODE_WHITE,
    tildes: false,
    mono: false,
};

/// Never bold: on the bold cursor line, some terminals let bold win over dim.
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
    lit: SyntaxColors {
        keyword: Color::Reset,
        kind: Color::Reset,
        function: Color::Reset,
        string: Color::Reset,
        number: Color::Reset,
        comment: Color::Reset,
        punctuation: Color::Reset,
        plain: Color::Reset,
        brackets: [Color::Reset; 3],
    },
    ghost: None,
    glow: None,
    status: Style::new().bg(Color::Reset).fg(Color::Reset),
    status_item: Style::new().bg(Color::Reset).fg(Color::Reset),
    status_alert: Color::Reset,
    tildes: true,
    mono: true,
};
