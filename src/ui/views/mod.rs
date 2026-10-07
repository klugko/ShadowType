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
    use unicode_width::UnicodeWidthStr;

    use super::{Modifier, Palette, Row, Span};
    use crate::ui::chart;

    /// Columns left free on the right of a chart.
    const CHART_MARGIN: u16 = 2;

    /// A line chart of `values`, `height` rows tall, in a text `width`
    /// columns wide.
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

    /// `key = value`, the key padded to `width` columns. The value is the
    /// last span.
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
                palette.fg(palette.text),
            ));
        }
        Row::new(spans)
    }
}
