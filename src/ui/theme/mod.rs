/*!
 * Colour palettes: `editor` and `vscode` (VS Code's Dark+) in true colours,
 * `dark` in the 16 ANSI colours on black, and `mono` without colour, telling
 * every state apart by bold, dim, underlined or reversed text instead.
 */

mod dark;
mod editor;
mod mono;
mod vscode;

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
            Theme::Editor => editor::PALETTE,
            Theme::Dark => dark::PALETTE,
            Theme::Mono => mono::PALETTE,
            Theme::VsCode => vscode::PALETTE,
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
        match token {
            Some(token) => self.token(token, self.lit.of(token)),
            None => self.fg(self.strong),
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
        self.token(token, color)
    }

    fn token(&self, token: Token, color: Color) -> Style {
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
