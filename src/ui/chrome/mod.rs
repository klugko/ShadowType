/*!
 * The editor frame around the buffer: explorer, tab line, status line and
 * command line.
 */

mod cmdline;
mod explorer;
mod statusline;
mod tabline;

use code_racer_engine::CodeLanguage;
use ratatui::{style::Color, text::Span};

use crate::{app::App, ui::theme::Palette};
pub use cmdline::{cmdline, cmdline_height};
pub use explorer::{shows_mascot, sidebar};
pub use statusline::statusline;
pub use tabline::tabline;

/// In discreet mode, the directory it runs in, as an editor names any project.
fn project_name(app: &App) -> &str {
    if app.config.discreet {
        &app.workspace
    } else {
        "code-racer"
    }
}

/**
 * The file type of a buffer named `name`, as the status line shows it,
 * and the colour of the name in the explorer, both from its extension.
 */
fn file_kind(name: &str, palette: &Palette) -> (&'static str, Color) {
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);
    match extension {
        _ if name == "COMMIT_EDITMSG" => ("gitcommit", palette.keyword),
        Some("toml") => ("toml", palette.kind),
        Some("md") => ("markdown", palette.accent),
        Some("log") => ("log", palette.string),
        Some("eml") => ("mail", palette.function),
        _ => match extension.and_then(CodeLanguage::from_extension) {
            Some(language) => (language.name(), palette.text),
            None => ("text", palette.text),
        },
    }
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(Span::width).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Theme;

    #[test]
    fn the_file_type_follows_the_extension_of_the_name_shown() {
        let palette = Palette::of(Theme::Editor);
        let names = [
            "race.toml",
            "help.md",
            "FK72AD.md",
            "history.log",
            "main.rs",
            "component.tsx",
            "query.sql",
            "scratch.txt",
            "Makefile",
        ];
        let types = names.map(|name| file_kind(name, &palette).0);
        assert_eq!(
            types,
            [
                "toml",
                "markdown",
                "markdown",
                "log",
                "rust",
                "typescript",
                "sql",
                "text",
                "text"
            ]
        );
    }
}
