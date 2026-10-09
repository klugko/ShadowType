//! File icons, as a code editor's file icon theme puts them in front of file
//! names: a glyph and a colour for each kind of file.
//!
//! The `unicode` set uses symbols every terminal font has, the `nerd` set
//! the icons of a Nerd Font. The colours are those of VS Code's Seti icons.

use code_racer_engine::CodeLanguage;
use ratatui::style::Color;

use crate::{config::Icons, ui::theme::Palette};

/// What a file holds, for its icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Config,
    Markdown,
    Log,
    Text,
    Commit,
    Mail,
    Code(CodeLanguage),
}

impl Kind {
    fn of(name: &str) -> Self {
        if name == "COMMIT_EDITMSG" {
            return Self::Commit;
        }
        let extension = name.rsplit_once('.').map(|(_, extension)| extension);
        match extension {
            Some("toml") => Self::Config,
            Some("md") => Self::Markdown,
            Some("log") => Self::Log,
            Some("eml") => Self::Mail,
            _ => extension
                .and_then(CodeLanguage::from_extension)
                .map_or(Self::Text, Self::Code),
        }
    }

    fn glyph(self, icons: Icons) -> Option<&'static str> {
        let (unicode, nerd) = match self {
            Self::Config => ("§", "\u{e615}"),
            Self::Markdown => ("¶", "\u{e609}"),
            Self::Log => ("≡", "\u{f03a}"),
            Self::Text => ("■", "\u{f0f6}"),
            Self::Commit => ("±", "\u{e702}"),
            Self::Mail => ("@", "\u{f0e0}"),
            Self::Code(language) => (
                "♦",
                match language {
                    CodeLanguage::Rust => "\u{e7a8}",
                    CodeLanguage::Python => "\u{e606}",
                    CodeLanguage::TypeScript => "\u{e628}",
                    CodeLanguage::JavaScript => "\u{e60c}",
                    CodeLanguage::Sql => "\u{e706}",
                },
            ),
        };
        match icons {
            Icons::Unicode => Some(unicode),
            Icons::Nerd => Some(nerd),
            Icons::None => None,
        }
    }

    /// The colour of the icon: Seti's in true colours, the nearest of the
    /// 16 colours otherwise.
    fn color(self, palette: &Palette) -> Color {
        let (rgb, ansi) = match self {
            Self::Config => ((109, 128, 134), Color::DarkGray),
            Self::Markdown | Self::Mail => ((81, 154, 186), Color::Cyan),
            Self::Log => ((203, 203, 65), Color::Yellow),
            Self::Text => ((212, 215, 214), Color::White),
            Self::Commit => ((227, 121, 51), Color::LightRed),
            Self::Code(CodeLanguage::Rust) => ((222, 165, 132), Color::LightRed),
            Self::Code(CodeLanguage::Python) => ((81, 154, 186), Color::Blue),
            Self::Code(CodeLanguage::TypeScript) => ((49, 120, 198), Color::Blue),
            Self::Code(CodeLanguage::JavaScript) => ((203, 203, 65), Color::Yellow),
            Self::Code(CodeLanguage::Sql) => ((245, 83, 133), Color::LightMagenta),
        };
        if palette.mono {
            Color::Reset
        } else if palette.blends() {
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        } else {
            ansi
        }
    }
}

/// The icon of the file `name` and its colour, `None` without icons.
pub fn file(name: &str, icons: Icons, palette: &Palette) -> Option<(&'static str, Color)> {
    let kind = Kind::of(name);
    kind.glyph(icons).map(|glyph| (glyph, kind.color(palette)))
}

/// The icon of an open folder, which only Nerd Fonts have.
pub fn folder(icons: Icons) -> Option<&'static str> {
    (icons == Icons::Nerd).then_some("\u{f07c}")
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthStr;

    use super::*;
    use crate::config::Theme;

    const NAMES: [&str; 9] = [
        "practice.toml",
        "help.md",
        "history.log",
        "scratch.txt",
        "COMMIT_EDITMSG",
        "draft.eml",
        "main.rs",
        "query.sql",
        "Makefile",
    ];

    #[test]
    fn every_icon_takes_one_column() {
        let palette = Palette::of(Theme::Editor);
        for icons in [Icons::Unicode, Icons::Nerd] {
            for name in NAMES {
                let (glyph, _) = file(name, icons, &palette).expect("an icon");
                assert_eq!(glyph.width(), 1, "{name} in {icons}");
            }
        }
    }

    #[test]
    fn icons_follow_the_kind_of_file() {
        let palette = Palette::of(Theme::VsCode);
        let glyph = |name| file(name, Icons::Unicode, &palette).map(|(glyph, _)| glyph);
        assert_eq!(glyph("practice.toml"), Some("§"));
        assert_eq!(glyph("notes.md"), Some("¶"));
        assert_eq!(glyph("COMMIT_EDITMSG"), Some("±"));
        assert_eq!(glyph("draft.eml"), Some("@"));
        assert_eq!(glyph("index.ts"), glyph("main.rs"), "code shares a shape");
        let color = |name| file(name, Icons::Unicode, &palette).map(|(_, color)| color);
        assert_ne!(
            color("index.ts"),
            color("main.rs"),
            "and its language a colour"
        );
        assert_eq!(file("main.rs", Icons::None, &palette), None);
    }

    #[test]
    fn icons_take_the_colours_the_theme_can_show() {
        let colour = |theme| file("main.rs", Icons::Unicode, &Palette::of(theme)).map(|(_, c)| c);
        assert!(matches!(colour(Theme::Editor), Some(Color::Rgb(..))));
        assert_eq!(colour(Theme::Dark), Some(Color::LightRed));
        assert_eq!(colour(Theme::Mono), Some(Color::Reset));
    }
}
