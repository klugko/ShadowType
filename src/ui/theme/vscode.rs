use ratatui::style::{Color, Modifier, Style};

use super::{Palette, SyntaxColors, block, underlined};

const BACKGROUND: Color = Color::Rgb(30, 30, 30);
const CURRENT_LINE: Color = Color::Rgb(40, 40, 40);
const FOREGROUND: Color = Color::Rgb(212, 212, 212);
/// Text still to type: 4.6:1 on the current line, typed text 2.1:1 on it.
const PENDING: Color = Color::Rgb(144, 144, 144);
const BLUE: Color = Color::Rgb(0, 122, 204);
const WHITE: Color = Color::Rgb(255, 255, 255);
const ERROR: Color = Color::Rgb(241, 76, 76);

pub(super) const PALETTE: Palette = Palette {
    background: BACKGROUND,
    panel: Color::Rgb(37, 37, 38),
    highlight: Color::Rgb(42, 45, 46),
    cursorline: Style::new().bg(CURRENT_LINE),
    cursor: block(BLUE, WHITE),
    mistake: underlined(ERROR),
    pending: Style::new().fg(PENDING),
    selection: Style::new()
        .bg(Color::Rgb(4, 57, 94))
        .fg(WHITE)
        .add_modifier(Modifier::BOLD),
    border: Color::Rgb(60, 60, 60),
    text: Color::Rgb(204, 204, 204),
    strong: FOREGROUND,
    muted: Color::Rgb(133, 133, 133),
    faint: Color::Rgb(110, 110, 110),
    accent: BLUE,
    success: Color::Rgb(137, 209, 133),
    warning: Color::Rgb(204, 167, 0),
    error: ERROR,
    insert: Color::Rgb(22, 130, 93),
    command: Color::Rgb(136, 23, 152),
    keyword: Color::Rgb(86, 156, 214),
    kind: Color::Rgb(78, 201, 176),
    function: Color::Rgb(156, 220, 254),
    string: Color::Rgb(206, 145, 120),
    number: Color::Rgb(181, 206, 168),
    comment: Color::Rgb(106, 153, 85),
    punctuation: FOREGROUND,
    on_accent: WHITE,
    lit: SyntaxColors {
        keyword: Color::Rgb(86, 156, 214),
        kind: Color::Rgb(78, 201, 176),
        function: Color::Rgb(220, 220, 170),
        string: Color::Rgb(206, 145, 120),
        number: Color::Rgb(181, 206, 168),
        comment: Color::Rgb(106, 153, 85),
        punctuation: FOREGROUND,
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
    status: Style::new().bg(BLUE).fg(WHITE),
    status_item: Style::new().bg(Color::Rgb(0, 95, 160)).fg(WHITE),
    status_alert: WHITE,
    tildes: false,
    mono: false,
};
