use ratatui::style::{Color, Style};

use super::{Palette, SyntaxColors, block, highlighted, underlined};

const HIGHLIGHT: Color = Color::Rgb(36, 40, 50);
const STRONG: Color = Color::Rgb(214, 219, 228);
const MUTED: Color = Color::Rgb(92, 99, 112);
/// Text still to type: dimmer than typed text, yet readable on the cursor line.
const PENDING: Color = Color::Rgb(138, 146, 160);
const ACCENT: Color = Color::Rgb(97, 166, 230);
const ERROR: Color = Color::Rgb(226, 108, 117);
const ON_ACCENT: Color = Color::Rgb(21, 24, 30);
const PANEL: Color = Color::Rgb(21, 24, 30);
const TEXT: Color = Color::Rgb(171, 178, 191);

pub(super) const PALETTE: Palette = Palette {
    background: Color::Rgb(26, 29, 36),
    panel: PANEL,
    highlight: HIGHLIGHT,
    cursorline: Style::new().bg(HIGHLIGHT),
    cursor: block(ACCENT, ON_ACCENT),
    mistake: underlined(ERROR),
    pending: Style::new().fg(PENDING),
    selection: highlighted(HIGHLIGHT, STRONG),
    border: Color::Rgb(44, 49, 60),
    text: TEXT,
    strong: STRONG,
    muted: MUTED,
    faint: Color::Rgb(70, 76, 89),
    accent: ACCENT,
    success: Color::Rgb(140, 190, 120),
    warning: Color::Rgb(222, 186, 112),
    error: ERROR,
    insert: Color::Rgb(140, 190, 120),
    command: Color::Rgb(222, 186, 112),
    keyword: Color::Rgb(194, 130, 220),
    kind: Color::Rgb(222, 186, 112),
    function: Color::Rgb(97, 166, 230),
    string: Color::Rgb(140, 190, 120),
    number: Color::Rgb(214, 150, 98),
    comment: Color::Rgb(106, 114, 128),
    punctuation: Color::Rgb(150, 158, 172),
    on_accent: ON_ACCENT,
    lit: SyntaxColors {
        keyword: Color::Rgb(226, 197, 239),
        kind: Color::Rgb(230, 204, 148),
        function: Color::Rgb(176, 211, 243),
        string: Color::Rgb(185, 216, 173),
        number: Color::Rgb(234, 201, 174),
        comment: Color::Rgb(178, 183, 191),
        punctuation: Color::Rgb(203, 207, 214),
        plain: STRONG,
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
        plain: PENDING,
        brackets: [
            Color::Rgb(164, 145, 41),
            Color::Rgb(190, 120, 188),
            Color::Rgb(69, 151, 209),
        ],
    }),
    glow: Some(ACCENT),
    status: Style::new().bg(PANEL).fg(TEXT),
    status_item: Style::new().bg(HIGHLIGHT).fg(STRONG),
    status_alert: ERROR,
    tildes: true,
    mono: false,
};
