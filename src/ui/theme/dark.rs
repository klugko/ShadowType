use ratatui::style::{Color, Modifier, Style};

use super::{Palette, SyntaxColors, block, highlighted, underlined};

const HIGHLIGHT: Color = Color::DarkGray;
const STRONG: Color = Color::White;
const MUTED: Color = Color::DarkGray;
const ACCENT: Color = Color::Blue;
const ERROR: Color = Color::Red;
const ON_ACCENT: Color = Color::Black;

pub(super) const PALETTE: Palette = Palette {
    background: Color::Black,
    panel: Color::Black,
    highlight: HIGHLIGHT,
    cursorline: Style::new().add_modifier(Modifier::BOLD),
    cursor: block(ACCENT, ON_ACCENT),
    mistake: underlined(ERROR),
    pending: Style::new().fg(MUTED),
    selection: highlighted(HIGHLIGHT, STRONG),
    border: Color::DarkGray,
    text: Color::Gray,
    strong: STRONG,
    muted: MUTED,
    faint: Color::DarkGray,
    accent: ACCENT,
    success: Color::Green,
    warning: Color::Yellow,
    error: ERROR,
    insert: Color::Green,
    command: Color::Yellow,
    keyword: Color::Magenta,
    kind: Color::Yellow,
    function: Color::Blue,
    string: Color::Green,
    number: Color::Cyan,
    comment: Color::DarkGray,
    punctuation: Color::Gray,
    on_accent: ON_ACCENT,
    lit: SyntaxColors {
        keyword: Color::Magenta,
        kind: Color::Yellow,
        function: Color::Blue,
        string: Color::Green,
        number: Color::Cyan,
        comment: Color::Gray,
        punctuation: Color::Gray,
        plain: STRONG,
        brackets: [Color::Yellow, Color::Magenta, Color::Cyan],
    },
    ghost: None,
    glow: None,
    status: Style::new().bg(Color::Black).fg(Color::Gray),
    status_item: Style::new().bg(HIGHLIGHT).fg(STRONG),
    status_alert: ERROR,
    tildes: true,
    mono: false,
};
