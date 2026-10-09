use code_racer_engine::Mark;
use ratatui::{
    style::{Color, Style},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use super::code::{Syntax, leading_spaces};
use crate::{
    app::{Disguise, SessionView, ink::DRYING_TIME},
    ui::{
        Moment,
        editor::Row,
        looks::{self, RowState},
        theme::{Palette, blend},
        wrap::VisualLine,
    },
};

/// How much of the cursor's colour its trail takes at its brightest.
const TRAIL_STRENGTH: f64 = 0.65;
const INDENT_GUIDE: &str = "│";

pub(super) struct Painter<'a> {
    pub(super) view: &'a SessionView<'a>,
    pub(super) palette: &'a Palette,
    pub(super) moment: Moment,
    pub(super) disguise: Option<Disguise<'a>>,
    pub(super) cursor: Option<usize>,
    pub(super) cursor_style: Style,
    pub(super) syntax: Option<Syntax>,
    /// The width of a level of indentation, in code, which shows guides.
    pub(super) indent: Option<usize>,
}

impl Painter<'_> {
    /**
     * The row of the text `line`, its `index`th, under the cursor when
     * `current`, the end of the text when `last`.
     */
    pub(super) fn row(&self, index: usize, line: &VisualLine, current: bool, last: bool) -> Row {
        let target = self.view.session.target();
        let state = row_state(self.view, index, line);
        let mut spans = self.disguise.map_or_else(Vec::new, |disguise| {
            looks::prefix(disguise, state, self.palette, self.moment)
        });
        let background = match self.palette.cursorline.bg {
            Some(cursorline) if current => cursorline,
            _ => self.palette.background,
        };
        let indentation = line.start + leading_spaces(target, line);
        for position in line.start..line.end {
            let guide = self.indent.is_some_and(|unit| {
                position < indentation && (position - line.start).is_multiple_of(unit)
            });
            let (glyph, style) = self.cell(position, guide);
            let mut style = self.trailing(position, style, background);
            if let Some(disguise) = self.disguise
                && self.cursor != Some(position)
            {
                style = looks::restyle(disguise, state, style, self.palette);
            }
            push_merged(&mut spans, (glyph, style));
        }
        if last && self.cursor == Some(target.len()) {
            spans.push(Span::styled(" ", self.cursor_style));
        }
        Row {
            number: line.number,
            current,
            ..Row::new(spans)
        }
    }

    /// A blank is drawn as an indentation guide when `guide`.
    fn cell(&self, index: usize, guide: bool) -> (String, Style) {
        let session = self.view.session;
        let mark = session.mark(index);
        let is_cursor = self.cursor == Some(index);
        let glyph = glyph(&session.target()[index], mark, is_cursor, guide);
        let style = if is_cursor {
            self.cursor_style
        } else if glyph == INDENT_GUIDE {
            guide_style(self.palette)
        } else {
            self.style(index, mark)
        };
        (glyph.to_owned(), style)
    }

    fn style(&self, index: usize, mark: Mark) -> Style {
        let palette = self.palette;
        let token = self.syntax.as_ref().map(|syntax| syntax.tokens[index]);
        let depth = self.syntax.as_ref().and_then(|syntax| syntax.depths[index]);
        match mark {
            Mark::Pending => match depth {
                Some(depth) => palette.bracket(depth, false),
                None => palette.ghost(token),
            },
            Mark::Correct => {
                let dry = match depth {
                    Some(depth) => palette.bracket(depth, true),
                    None => palette.typed(token),
                };
                if self.trail_at(index) > 0.0 {
                    dry
                } else {
                    self.drying(index, dry)
                }
            }
            Mark::Incorrect => palette.mistake,
        }
    }

    /**
     * How much of the cursor's trail lies on the character at `index`, 0
     * when no trail is drawn.
     */
    fn trail_at(&self, index: usize) -> f64 {
        match self.view.ink {
            Some(ink) if self.moment.trail && self.palette.blends() => {
                ink.trail(index, self.moment.now)
            }
            _ => 0.0,
        }
    }

    /**
     * `style` over the trail of the cursor at `index`, if it passed there
     * lately: a streak of its colour fading into `background`.
     */
    fn trailing(&self, index: usize, style: Style, background: Color) -> Style {
        if self.cursor == Some(index) {
            return style;
        }
        let strength = self.trail_at(index);
        match self.palette.cursor.bg {
            Some(head) if strength > 0.0 => {
                style.bg(blend(background, head, strength * TRAIL_STRENGTH))
            }
            _ => style,
        }
    }

    /// `dry`, glowing still when the character at `index` was just typed.
    fn drying(&self, index: usize, dry: Style) -> Style {
        let (Some(glow), Some(ink), Some(color), true) = (
            self.palette.glow,
            self.view.ink,
            dry.fg,
            self.moment.animate,
        ) else {
            return dry;
        };
        let Some(at) = ink.typed_at(index) else {
            return dry;
        };
        let age = self.moment.now.saturating_duration_since(at);
        if age >= DRYING_TIME {
            return dry;
        }
        let progress = age.as_secs_f64() / DRYING_TIME.as_secs_f64();
        let eased = 1.0 - (1.0 - progress).powi(3);
        dry.fg(blend(glow, color, eased))
    }
}

fn glyph(grapheme: &str, mark: Mark, is_cursor: bool, guide: bool) -> &str {
    match grapheme {
        "\n" if is_cursor || mark == Mark::Incorrect => "↵",
        "\n" => " ",
        " " if mark == Mark::Incorrect => "·",
        " " if guide && !is_cursor => INDENT_GUIDE,
        invisible if invisible.width() == 0 => "◌",
        other => other,
    }
}

fn row_state(view: &SessionView<'_>, index: usize, line: &VisualLine) -> RowState {
    let session = view.session;
    let done = line.end > line.start
        && (line.start..line.end).all(|position| session.mark(position) == Mark::Correct);
    let ink = view.ink;
    RowState {
        index,
        done,
        started: ink.and_then(|ink| ink.wall_time(line.start)),
        started_at: ink.and_then(|ink| ink.typed_at(line.start)),
        done_at: ink
            .filter(|_| done)
            .and_then(|ink| ink.typed_at(line.end - 1)),
    }
}

fn guide_style(palette: &Palette) -> Style {
    if palette.mono {
        palette.pending
    } else {
        palette.fg(palette.faint)
    }
}

fn push_merged(spans: &mut Vec<Span<'static>>, (text, style): (String, Style)) {
    match spans.last_mut() {
        Some(last) if last.style == style => last.content.to_mut().push_str(&text),
        _ => spans.push(Span::styled(text, style)),
    }
}
