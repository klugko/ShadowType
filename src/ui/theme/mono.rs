use ratatui::style::{Color, Modifier, Style};

use super::{Palette, SyntaxColors};

/// Never bold: on the bold cursor line, some terminals let bold win over dim.
const PENDING: Style = Style::new()
    .add_modifier(Modifier::DIM)
    .remove_modifier(Modifier::BOLD);

pub(super) const PALETTE: Palette = Palette {
    background: Color::Reset,
    panel: Color::Reset,
    highlight: Color::Reset,
    cursorline: Style::new().add_modifier(Modifier::BOLD),
    cursor: Style::new().add_modifier(Modifier::REVERSED),
    mistake: Style::new().add_modifier(Modifier::UNDERLINED.union(Modifier::BOLD)),
    pending: PENDING,
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
