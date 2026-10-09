use ratatui::text::Span;
use unicode_normalization::UnicodeNormalization;

use crate::ui::{editor::Row, theme::Palette};

pub(super) fn header(name: &str, palette: &Palette) -> Vec<Row> {
    let author = author(name);
    let field = |key: &str, value: String| {
        Row::new(vec![
            Span::styled(format!("{key}:"), palette.fg(palette.keyword)),
            Span::styled(format!(" {value}"), palette.fg(palette.text)),
        ])
    };
    vec![
        field("From", format!("{author} <{}@localhost>", address(&author))),
        field("To", "team@lists.internal".to_owned()),
        field("Subject", "Re: notes from the sync".to_owned()),
        Row::blank(),
        plain("Hi all,", palette),
        Row::blank(),
    ]
}

pub(super) fn footer(name: &str, palette: &Palette) -> Vec<Row> {
    vec![
        Row::blank(),
        plain("Best,", palette),
        plain(&author(name), palette),
    ]
}

fn plain(text: &str, palette: &Palette) -> Row {
    Row::new(vec![Span::styled(
        text.to_owned(),
        palette.fg(palette.text),
    )])
}

fn author(name: &str) -> String {
    match name.trim() {
        "" => "me".to_owned(),
        name => name.to_owned(),
    }
}

/**
 * The local part of an email address for `name`: its letters without
 * their accents, a dot for each space.
 */
fn address(name: &str) -> String {
    let address: String = name
        .to_lowercase()
        .nfd()
        .map(|ch| if ch.is_whitespace() { '.' } else { ch })
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        .collect();
    if address.is_empty() {
        "me".to_owned()
    } else {
        address
    }
}
