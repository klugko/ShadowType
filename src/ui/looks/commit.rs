use ratatui::text::Span;

use crate::ui::{editor::Row, theme::Palette};

/// The template git puts under a commit message.
pub(super) fn footer(palette: &Palette) -> Vec<Row> {
    let comment = |text: &str| {
        Row::new(vec![Span::styled(
            text.to_owned(),
            palette.fg(palette.comment),
        )])
    };
    let file = |path: &str| {
        Row::new(vec![
            Span::styled("#       modified:   ", palette.fg(palette.comment)),
            Span::styled(path.to_owned(), palette.fg(palette.string)),
        ])
    };
    vec![
        Row::blank(),
        comment("# Please enter the commit message for your changes. Lines starting"),
        comment("# with '#' will be ignored, and an empty message aborts the commit."),
        comment("#"),
        Row::new(vec![
            Span::styled("# On branch ", palette.fg(palette.comment)),
            Span::styled("main", palette.fg(palette.function)),
        ]),
        comment("# Your branch is up to date with 'origin/main'."),
        comment("#"),
        comment("# Changes to be committed:"),
        file("src/lib.rs"),
        file("README.md"),
        comment("#"),
    ]
}
