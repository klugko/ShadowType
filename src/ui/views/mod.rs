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
    app::{Activity, App, Buffer, mouse::Target},
    ui::{editor::Row, hits, theme::Palette},
};

/// Draws the selected buffer; returns where the terminal cursor goes, if anywhere.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &Palette,
    now: Instant,
) -> Option<Position> {
    hits::mark(area, Target::Editor);
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
                Some(Activity::Solo(run)) => {
                    let moment = crate::ui::Moment::of_app(app, now);
                    session::render(frame, area, app, run, palette, moment);
                }
                Some(Activity::Race(client)) => {
                    room::render(frame, area, app, client, palette, now)
                }
                None => {}
            }
            None
        }
    }
}

/**
 * Whether the buffer on screen moves at `moment`, its text aside: the
 * results of a solo session coming up.
 */
pub fn is_moving(app: &App, moment: crate::ui::Moment) -> bool {
    app.buffer == Buffer::Session
        && app
            .solo()
            .and_then(|run| run.result.as_ref())
            .is_some_and(|result| session::reveals(result, moment))
}

/// Small builders for markdown and TOML looking lines.
mod doc {
    use crossterm::event::KeyCode;
    use unicode_width::UnicodeWidthStr;

    use super::{Modifier, Palette, Row, Span};
    use crate::{app::mouse::Target, ui::chart};

    /// Columns left free on the right of a chart.
    const CHART_MARGIN: u16 = 2;

    pub fn chart(values: &[f64], width: u16, height: u16, palette: &Palette) -> Vec<Row> {
        chart::line_chart(values, width.saturating_sub(CHART_MARGIN), height)
            .into_iter()
            .map(|line| Row::new(vec![Span::styled(line, palette.fg(palette.accent))]))
            .collect()
    }

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

    /// What separates the key of an [`assignment`] from its value.
    const EQUALS: &str = " = ";

    /**
     * `key = value`, the key padded to `width` columns. The value is the
     * last span.
     */
    pub fn assignment(
        key: &str,
        width: usize,
        value: Span<'static>,
        palette: &Palette,
    ) -> Vec<Span<'static>> {
        vec![
            Span::styled(padded_key(key, width), palette.fg(palette.function)),
            Span::styled(EQUALS, palette.fg(palette.punctuation)),
            value,
        ]
    }

    /// The column the value of an [`assignment`] starts at.
    pub fn value_column(key: &str, width: usize) -> usize {
        padded_key(key, width).width() + EQUALS.width()
    }

    fn padded_key(key: &str, width: usize) -> String {
        format!("{key:<width$}")
    }

    /// What opens and closes a [`string`].
    pub const QUOTE: &str = "\"";

    pub fn string(value: &str, palette: &Palette) -> Span<'static> {
        Span::styled(format!("{QUOTE}{value}{QUOTE}"), palette.fg(palette.string))
    }

    /// What separates two keys of a [`keys`] row.
    const KEY_GAP: &str = "    ";

    /**
     * Keys and their effect, such as `r  toggle ready`. A click on one
     * presses its key.
     */
    pub fn keys(bindings: &[(&str, &str)], palette: &Palette) -> Row {
        let mut spans = Vec::new();
        let mut keys = Vec::new();
        let mut column = 0;
        for (index, (key, action)) in bindings.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw(KEY_GAP));
                column += KEY_GAP.width();
            }
            let width = key.width() + 2 + action.width();
            if let Some(target) = key_target(key) {
                keys.push((
                    u16::try_from(column).unwrap_or(u16::MAX),
                    u16::try_from(width).unwrap_or(u16::MAX),
                    target,
                ));
            }
            column += width;
            spans.push(Span::styled(
                (*key).to_owned(),
                palette.fg(palette.accent).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!("  {action}"),
                palette.fg(palette.text),
            ));
        }
        Row {
            keys,
            ..Row::new(spans)
        }
    }

    /**
     * What a click on a key written `label` presses: `Esc Esc` is Esc
     * twice.
     */
    fn key_target(label: &str) -> Option<Target> {
        let mut presses = label.split_whitespace();
        let first = presses.next()?;
        let times = u8::try_from(presses.count() + 1).ok()?;
        let code = match first {
            "Esc" => KeyCode::Esc,
            "Enter" => KeyCode::Enter,
            "Space" => KeyCode::Char(' '),
            key => {
                let mut chars = key.chars();
                let ch = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                KeyCode::Char(ch)
            }
        };
        Some(Target::Key { code, times })
    }
}
