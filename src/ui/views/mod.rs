//! Content of the editor pane, one module per kind of buffer.

mod forms;
mod help;
mod history;
mod room;
mod session;

use std::time::Instant;

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Modifier,
    text::Span,
};

use crate::{
    app::{Activity, App, Buffer},
    ui::{editor::Row, theme::Palette},
};

/// Draws the selected buffer; returns where the terminal cursor goes, if anywhere.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
    now: Instant,
) -> Option<Position> {
    match app.buffer {
        Buffer::Practice => forms::practice(frame, area, app, palette),
        Buffer::Race => forms::race(frame, area, app, palette),
        Buffer::Settings => forms::settings(frame, area, app, palette),
        Buffer::History => {
            history::render(frame, area, app, palette);
            None
        }
        Buffer::Help => {
            help::render(frame, area, app.help_scroll, palette);
            None
        }
        Buffer::Session => {
            match &app.activity {
                Some(Activity::Solo(run)) => session::render(frame, area, app, run, palette),
                Some(Activity::Race(client)) => {
                    room::render(frame, area, app, client, palette, now)
                }
                None => {}
            }
            None
        }
    }
}

/// Small builders for markdown and TOML looking lines.
mod doc {
    use super::{Modifier, Palette, Row, Span};

    pub fn comment(text: impl Into<String>, palette: &Palette) -> Row {
        Row::new(vec![Span::styled(
            format!("# {}", text.into()),
            palette.fg(palette.comment),
        )])
    }

    pub fn title(text: impl Into<String>, palette: &Palette) -> Row {
        Row::new(vec![Span::styled(
            format!("# {}", text.into()),
            palette.fg(palette.keyword).add_modifier(Modifier::BOLD),
        )])
    }

    pub fn heading(text: impl Into<String>, palette: &Palette) -> Row {
        Row::new(vec![Span::styled(
            format!("## {}", text.into()),
            palette.fg(palette.accent).add_modifier(Modifier::BOLD),
        )])
    }

    pub fn text(text: impl Into<String>, palette: &Palette) -> Row {
        Row::new(vec![Span::styled(text.into(), palette.fg(palette.text))])
    }

    pub fn blank() -> Row {
        Row::blank()
    }

    /// `key = "value"` aligned on `width`, with a trailing comment.
    pub fn assignment(
        key: &str,
        width: usize,
        value: Span<'static>,
        palette: &Palette,
    ) -> Vec<Span<'static>> {
        vec![
            Span::styled(format!("{key:<width$}"), palette.fg(palette.function)),
            Span::styled(" = ", palette.fg(palette.punctuation)),
            value,
        ]
    }

    pub fn string(value: &str, palette: &Palette) -> Span<'static> {
        Span::styled(format!("\"{value}\""), palette.fg(palette.string))
    }

    /// Keys and their effect, such as `r  toggle ready`.
    pub fn keys(bindings: &[(&str, &str)], palette: &Palette) -> Row {
        let mut spans = Vec::new();
        for (index, (key, action)) in bindings.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("    "));
            }
            spans.push(Span::styled(
                (*key).to_owned(),
                palette.fg(palette.accent).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!("  {action}"),
                palette.fg(palette.muted),
            ));
        }
        Row::new(spans)
    }
}

/// `mm:ss.t` race time.
fn race_time(milliseconds: u64) -> String {
    let tenths = milliseconds / 100;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// `1st`, `2nd`, `3rd`, `4th`…
fn ordinal(place: usize) -> String {
    let suffix = match (place % 10, place % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{place}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn race_times_use_minutes_seconds_and_tenths() {
        assert_eq!(race_time(41_234), "0:41.2");
        assert_eq!(race_time(61_000), "1:01.0");
    }

    #[test]
    fn ordinals_handle_the_teens() {
        let places: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 101].map(ordinal).to_vec();
        assert_eq!(
            places,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "101st"
            ]
        );
    }
}
