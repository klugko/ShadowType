use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{Activity, App, Buffer, EditorMode, Focus, MessageKind, TextField},
    ui::{format, theme::Palette, wrap},
};

const MAX_MESSAGE_ROWS: usize = 4;
const PROMPT: &str = ":";

type KeyHint = (&'static str, &'static str);

/// Returns the cursor position while a command is typed.
pub fn cmdline(frame: &mut Frame, area: Rect, app: &App, palette: &Palette) -> Option<Position> {
    if let Some(prompt) = &app.prompt {
        let width = usize::from(area.width).saturating_sub(PROMPT.width());
        let input = format::input_view(&prompt.input, width);
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("{PROMPT}{}", input.text),
                palette.fg(palette.strong),
            )),
            area,
        );
        let column = PROMPT.width() + input.cursor;
        return Some(Position {
            x: area.x
                + u16::try_from(column)
                    .unwrap_or(u16::MAX)
                    .min(area.width.saturating_sub(1)),
            y: area.y,
        });
    }
    let lines = match app.message() {
        Some(message) => message_rows(&message.text, area.width)
            .into_iter()
            .map(|row| Line::from(Span::styled(row, message_style(message.kind, palette))))
            .collect(),
        None => vec![hints(app, palette)],
    };
    frame.render_widget(Paragraph::new(lines), area);
    None
}

/**
 * As many rows as the message on screen needs, so that a long one, such as a
 * warning with the path of a backup, shows whole.
 */
pub fn cmdline_height(app: &App, width: u16) -> u16 {
    let rows = match (&app.prompt, app.message()) {
        (None, Some(message)) => message_rows(&message.text, width).len(),
        _ => 1,
    };
    u16::try_from(rows.max(1)).unwrap_or(1)
}

fn message_rows(text: &str, width: u16) -> Vec<String> {
    let graphemes: Vec<String> = text.graphemes(true).map(str::to_owned).collect();
    wrap::wrap(&graphemes, width)
        .into_iter()
        .filter(|row| row.start < row.end)
        .take(MAX_MESSAGE_ROWS)
        .map(|row| graphemes[row.start..row.end].concat().trim_end().to_owned())
        .collect()
}

fn message_style(kind: MessageKind, palette: &Palette) -> Style {
    match kind {
        MessageKind::Error => palette.fg(palette.error).add_modifier(Modifier::BOLD),
        MessageKind::Info => palette.fg(palette.text),
    }
}

fn hints(app: &App, palette: &Palette) -> Line<'static> {
    let mode = match app.editor_mode() {
        EditorMode::Insert => Span::styled(
            "-- INSERT --  ",
            palette.fg(palette.strong).add_modifier(Modifier::BOLD),
        ),
        _ => Span::raw(""),
    };
    let keys = key_hints(app);
    let mut spans = vec![mode];
    for (index, (key, action)) in keys.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            (*key).to_owned(),
            palette.fg(palette.strong).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {action}"), palette.fg(palette.text)));
    }
    Line::from(spans)
}

fn key_hints(app: &App) -> Vec<KeyHint> {
    if app.palette.is_some() {
        return vec![("↑↓", "select"), ("Enter", "run"), ("Esc", "close")];
    }
    if let Some(edit) = &app.editing {
        let action = match edit.field {
            TextField::RoomCode => "join",
            TextField::Username | TextField::Server => "save",
        };
        return vec![("Enter", action), ("Esc", "cancel")];
    }
    if app.is_typing() {
        return typing_hints(app);
    }
    if app.focus == Focus::Explorer {
        return vec![
            ("j/k", "move"),
            ("Enter", "open"),
            ("s", "solo"),
            ("Ctrl+P", "commands"),
            ("m", "race"),
            ("?", "help"),
            ("q", "quit"),
        ];
    }
    buffer_hints(app)
}

fn typing_hints(app: &App) -> Vec<KeyHint> {
    match app.activity {
        _ if app.config.discreet => Vec::new(),
        Some(Activity::Race(_)) => vec![("Esc Esc", "leave race")],
        _ => vec![
            ("Esc Esc", "abandon"),
            ("Ctrl+R", "restart"),
            ("Ctrl+W", "delete word"),
        ],
    }
}

fn buffer_hints(app: &App) -> Vec<KeyHint> {
    match app.buffer {
        Buffer::Practice | Buffer::Race | Buffer::Settings => vec![
            ("j/k", "move"),
            ("h/l", "change"),
            ("Enter", "select"),
            ("Esc", "explorer"),
            ("Ctrl+P", "commands"),
        ],
        Buffer::History | Buffer::Help => vec![
            ("j/k", "scroll"),
            ("g/G", "top/bottom"),
            ("Esc", "explorer"),
        ],
        Buffer::Session => match app.activity {
            Some(Activity::Race(_)) => {
                vec![("Esc", "leave"), ("Ctrl+P", "commands"), ("?", "help")]
            }
            _ => vec![
                ("r", "new text"),
                ("Esc", "close"),
                ("Ctrl+P", "commands"),
                ("?", "help"),
            ],
        },
    }
}
