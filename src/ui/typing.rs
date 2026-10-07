//! The text being typed: ghost text ahead of the cursor, real code behind it.

use code_racer_engine::{Mark, TypingSession};
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::SessionView,
    ui::{
        editor::Row,
        syntax::{self, Token},
        theme::Palette,
        wrap,
    },
};

/// Rows of the session text and the position of the typing cursor.
#[derive(Debug)]
pub struct TypingLayout {
    pub rows: Vec<Row>,
    pub cursor_row: usize,
}

/// Lays out the session text in `width` columns.
///
/// Prose is numbered row by row like a soft-wrapped file, code keeps its own
/// line numbers. The cursor block is drawn only while `active`.
pub fn layout(view: &SessionView<'_>, width: u16, palette: &Palette, active: bool) -> TypingLayout {
    let session = view.session;
    let target = session.target();
    let lines = wrap::wrap(target, width);
    let tokens = view
        .syntax
        .map(|language| syntax::highlight(target, language));
    let prose = !target.iter().any(|grapheme| grapheme == "\n");
    let cursor = active.then(|| session.cursor());
    let (cursor_row, _) = wrap::locate(target, &lines, session.cursor());
    let rows = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let mut spans = Vec::new();
            for position in line.start..line.end {
                let token = tokens
                    .as_ref()
                    .and_then(|tokens| tokens.get(position))
                    .copied();
                let cell = cell(session, position, token, cursor == Some(position), palette);
                push_merged(&mut spans, cell);
            }
            if cursor == Some(target.len()) && index + 1 == lines.len() {
                spans.push(Span::styled(" ", cursor_style(palette)));
            }
            Row {
                number: if prose { Some(index + 1) } else { line.number },
                spans,
                current: index == cursor_row,
            }
        })
        .collect();
    TypingLayout { rows, cursor_row }
}

fn cell(
    session: &TypingSession,
    index: usize,
    token: Option<Token>,
    is_cursor: bool,
    palette: &Palette,
) -> (String, Style) {
    let grapheme = session.target()[index].as_str();
    let mark = session.mark(index);
    let glyph = match grapheme {
        "\n" if is_cursor || mark == Mark::Incorrect => "↵",
        "\n" => " ",
        " " if mark == Mark::Incorrect => "·",
        invisible if invisible.width() == 0 => "◌",
        other => other,
    };
    let style = if is_cursor {
        cursor_style(palette)
    } else {
        match mark {
            Mark::Pending => palette.fg(palette.muted),
            Mark::Correct => {
                token.map_or_else(|| palette.fg(palette.strong), |token| palette.syntax(token))
            }
            Mark::Incorrect => error_style(palette),
        }
    };
    (glyph.to_owned(), style)
}

fn push_merged(spans: &mut Vec<Span<'static>>, (text, style): (String, Style)) {
    match spans.last_mut() {
        Some(last) if last.style == style => last.content.to_mut().push_str(&text),
        _ => spans.push(Span::styled(text, style)),
    }
}

fn cursor_style(palette: &Palette) -> Style {
    if palette.mono {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new().fg(palette.on_accent).bg(palette.accent)
    }
}

fn error_style(palette: &Palette) -> Style {
    if palette.mono {
        Style::new().add_modifier(Modifier::REVERSED | Modifier::UNDERLINED)
    } else {
        Style::new()
            .fg(palette.error)
            .underline_color(palette.error)
            .add_modifier(Modifier::UNDERLINED)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use code_racer_engine::{CodeLanguage, SessionOptions};

    use super::*;
    use crate::config::Theme;

    fn text_of(row: &Row) -> String {
        row.spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn view(session: &TypingSession, syntax: Option<CodeLanguage>) -> SessionView<'_> {
        SessionView {
            session,
            syntax,
            attribution: None,
            stopped_at: None,
        }
    }

    #[test]
    fn prose_rows_are_all_numbered() {
        let session = TypingSession::new("one two three four five six", SessionOptions::default());
        let layout = layout(&view(&session, None), 10, &Palette::of(Theme::Editor), true);
        assert!(layout.rows.len() > 1);
        let numbers: Vec<Option<usize>> = layout.rows.iter().map(|row| row.number).collect();
        assert_eq!(numbers[1], Some(2));
    }

    #[test]
    fn code_keeps_logical_line_numbers_and_shows_the_cursor_newline() {
        let mut session = TypingSession::new("fn a() {\n}", SessionOptions::default());
        let now = Instant::now();
        for ch in "fn a() {".chars() {
            session.type_char(ch, now);
        }
        let layout = layout(
            &view(&session, Some(CodeLanguage::Rust)),
            40,
            &Palette::of(Theme::Editor),
            true,
        );
        assert_eq!(layout.rows.len(), 2);
        assert_eq!(layout.rows[1].number, Some(2));
        assert!(text_of(&layout.rows[0]).ends_with('↵'));
        assert_eq!(layout.cursor_row, 0);
    }

    #[test]
    fn mistakes_show_the_expected_character() {
        let mut session = TypingSession::new("a b", SessionOptions::default());
        let now = Instant::now();
        session.type_char('a', now);
        session.type_char('x', now);
        let palette = Palette::of(Theme::Editor);
        let layout = layout(&view(&session, None), 20, &palette, true);
        let row = &layout.rows[0];
        assert_eq!(text_of(row), "a·b");
        let error = row
            .spans
            .iter()
            .find(|span| span.content == "·")
            .expect("error span");
        assert_eq!(error.style.fg, Some(palette.error));
    }

    #[test]
    fn zero_width_characters_get_a_visible_placeholder() {
        let session = TypingSession::new("a\u{200b}b", SessionOptions::default());
        let layout = layout(
            &view(&session, None),
            20,
            &Palette::of(Theme::Editor),
            false,
        );
        assert_eq!(text_of(&layout.rows[0]), "a◌b");
    }

    #[test]
    fn identical_styles_are_merged_into_one_span() {
        let session = TypingSession::new("abcdef", SessionOptions::default());
        let layout = layout(
            &view(&session, None),
            20,
            &Palette::of(Theme::Editor),
            false,
        );
        assert_eq!(layout.rows[0].spans.len(), 1);
    }

    #[test]
    fn finished_text_shows_a_cursor_after_the_end_only_while_active() {
        let mut session = TypingSession::new("ab", SessionOptions::default());
        let now = Instant::now();
        session.type_char('a', now);
        session.type_char('b', now);
        let palette = Palette::of(Theme::Editor);
        assert_eq!(
            text_of(&layout(&view(&session, None), 20, &palette, false).rows[0]),
            "ab"
        );
        let active = layout(&view(&session, None), 20, &palette, true);
        let row = &active.rows[0];
        assert_eq!(text_of(row), "ab ");
        let end = row.spans.last().expect("the cursor");
        assert_eq!(
            (end.content.as_ref(), end.style),
            (" ", cursor_style(&palette))
        );
    }
}
