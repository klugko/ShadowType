//! The text being typed: ghost text ahead of the cursor, real text behind it.
//! Code shows its syntax from the start, dimmed until typed; prose is dressed
//! as the kind of file its look makes it.

use std::{f64::consts::TAU, time::Duration};

use code_racer_engine::{CodeLanguage, Mark, Status};
use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{Disguise, SessionView, ink::DRYING_TIME},
    ui::{
        Moment,
        editor::Row,
        looks::{self, RowState},
        syntax::{self, Token},
        theme::{Palette, blend},
        wrap::{self, VisualLine},
    },
};

/// How long the cursor stays still after a key before it starts breathing.
const BREATHE_AFTER: Duration = Duration::from_millis(600);
const BREATH: Duration = Duration::from_millis(1_400);
/// How far the cursor fades into the line at the bottom of a breath.
const BREATH_DEPTH: f64 = 0.75;
/// How much of the cursor's colour its trail takes at its brightest.
const TRAIL_STRENGTH: f64 = 0.65;
const INDENT_GUIDE: &str = "│";
const DEFAULT_INDENT: usize = 4;

#[derive(Debug)]
pub struct TypingLayout {
    pub rows: Vec<Row>,
    pub cursor_row: usize,
}

/// Prose is numbered row by row like a soft-wrapped file, with the lines its
/// look puts around it; code keeps its own line numbers. The cursor block is
/// drawn only while `active`.
pub fn layout(
    view: &SessionView<'_>,
    width: u16,
    palette: &Palette,
    active: bool,
    moment: Moment,
) -> TypingLayout {
    let target = view.session.target();
    let prose = !target.iter().any(|grapheme| grapheme == "\n");
    let disguise = view.disguise.filter(|_| view.syntax.is_none());
    let prefix = disguise.map_or(0, looks::prefix_width);
    let lines = wrap::wrap(target, width.saturating_sub(prefix).max(1));
    let painter = Painter {
        view,
        palette,
        moment,
        disguise,
        cursor: active.then(|| view.session.cursor()),
        cursor_style: cursor_style(view, palette, moment),
        syntax: view.syntax.map(|language| Syntax::of(target, language)),
        indent: (!prose).then(|| indent_unit(target, &lines)),
    };
    let (cursor_line, _) = wrap::locate(target, &lines, view.session.cursor());
    let mut rows = disguise.map_or_else(Vec::new, |disguise| looks::header(disguise, palette));
    let cursor_row = rows.len() + cursor_line;
    for (index, line) in lines.iter().enumerate() {
        let last = index + 1 == lines.len();
        rows.push(painter.row(index, line, index == cursor_line, last));
    }
    if let Some(source) = view.attribution {
        rows.push(Row::blank());
        rows.push(Row::new(vec![Span::styled(
            format!("-- {source}"),
            palette.fg(palette.comment).add_modifier(Modifier::ITALIC),
        )]));
    }
    if let Some(disguise) = disguise {
        rows.extend(looks::footer(disguise, palette));
    }
    if prose {
        for (number, row) in (1..).zip(&mut rows) {
            row.number = Some(number);
        }
    }
    TypingLayout { rows, cursor_row }
}

/// Whether the text of `view` moves at `moment`: fresh ink drying, the
/// cursor trailing, or a running text, whose cursor breathes.
pub fn is_moving(view: &SessionView<'_>, moment: Moment) -> bool {
    let Some(ink) = view.ink.filter(|_| moment.animate) else {
        return false;
    };
    ink.is_wet(moment.now)
        || (moment.trail && ink.is_trailing(moment.now))
        || view.session.status() == Status::Running
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

/// The syntax class and bracket depth of every character of code.
struct Syntax {
    tokens: Vec<Token>,
    depths: Vec<Option<usize>>,
}

impl Syntax {
    fn of(target: &[String], language: CodeLanguage) -> Self {
        let tokens = syntax::highlight(target, language);
        let depths = bracket_depths(target, &tokens);
        Self { tokens, depths }
    }
}

struct Painter<'a> {
    view: &'a SessionView<'a>,
    palette: &'a Palette,
    moment: Moment,
    disguise: Option<Disguise<'a>>,
    cursor: Option<usize>,
    cursor_style: Style,
    syntax: Option<Syntax>,
    /// The width of a level of indentation, in code, which shows guides.
    indent: Option<usize>,
}

impl Painter<'_> {
    /// The row of the text `line`, its `index`th, under the cursor when
    /// `current`, the end of the text when `last`.
    fn row(&self, index: usize, line: &VisualLine, current: bool, last: bool) -> Row {
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
        let palette = self.palette;
        let grapheme = session.target()[index].as_str();
        let mark = session.mark(index);
        let is_cursor = self.cursor == Some(index);
        let glyph = match grapheme {
            "\n" if is_cursor || mark == Mark::Incorrect => "↵",
            "\n" => " ",
            " " if mark == Mark::Incorrect => "·",
            " " if guide && !is_cursor => INDENT_GUIDE,
            invisible if invisible.width() == 0 => "◌",
            other => other,
        };
        if is_cursor {
            return (glyph.to_owned(), self.cursor_style);
        }
        let token = self.syntax.as_ref().map(|syntax| syntax.tokens[index]);
        let depth = self.syntax.as_ref().and_then(|syntax| syntax.depths[index]);
        let style = match mark {
            _ if glyph == INDENT_GUIDE => guide_style(palette),
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
        };
        (glyph.to_owned(), style)
    }

    /// How much of the cursor's trail lies on the character at `index`, 0
    /// when no trail is drawn.
    fn trail_at(&self, index: usize) -> f64 {
        match self.view.ink {
            Some(ink) if self.moment.trail && self.palette.blends() => {
                ink.trail(index, self.moment.now)
            }
            _ => 0.0,
        }
    }

    /// `style` over the trail of the cursor at `index`, if it passed there
    /// lately: a streak of its colour fading into `background`.
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

/// The cursor, breathing when the player pauses in a running text, in
/// themes where colours blend.
fn cursor_style(view: &SessionView<'_>, palette: &Palette, moment: Moment) -> Style {
    let still = palette.cursor;
    let (Some(ink), true, true) = (view.ink, moment.animate, palette.blends()) else {
        return still;
    };
    let (Some(last), Status::Running) = (ink.last_key(), view.session.status()) else {
        return still;
    };
    let pause = moment.now.saturating_duration_since(last);
    let Some(breathing) = pause.checked_sub(BREATHE_AFTER) else {
        return still;
    };
    let phase = (breathing.as_millis() % BREATH.as_millis()) as f64 / BREATH.as_millis() as f64;
    let fade = (1.0 - (phase * TAU).cos()) / 2.0 * BREATH_DEPTH;
    let (Some(fg), Some(bg)) = (still.fg, still.bg) else {
        return still;
    };
    let pending = palette.pending.fg.unwrap_or(fg);
    still
        .fg(blend(fg, pending, fade))
        .bg(blend(bg, palette.highlight, fade))
}

fn guide_style(palette: &Palette) -> Style {
    if palette.mono {
        palette.pending
    } else {
        palette.fg(palette.faint)
    }
}

/// How deep each bracket of code is nested, `None` for other characters.
/// Brackets in strings and comments are not punctuation, so they do not count.
fn bracket_depths(target: &[String], tokens: &[Token]) -> Vec<Option<usize>> {
    let mut depth: usize = 0;
    target
        .iter()
        .zip(tokens)
        .map(|(grapheme, token)| {
            if *token != Token::Punctuation {
                return None;
            }
            match grapheme.as_str() {
                "(" | "[" | "{" => {
                    depth += 1;
                    Some(depth - 1)
                }
                ")" | "]" | "}" => {
                    depth = depth.saturating_sub(1);
                    Some(depth)
                }
                _ => None,
            }
        })
        .collect()
}

/// The width of one level of indentation in code: the smallest indentation
/// of its lines, or [`DEFAULT_INDENT`] when none is indented.
fn indent_unit(target: &[String], lines: &[VisualLine]) -> usize {
    lines
        .iter()
        .filter(|line| line.number.is_some())
        .map(|line| leading_spaces(target, line))
        .filter(|spaces| *spaces > 0)
        .min()
        .unwrap_or(DEFAULT_INDENT)
}

/// Zero for the rows that continue a wrapped line.
fn leading_spaces(target: &[String], line: &VisualLine) -> usize {
    if line.number.is_none() {
        return 0;
    }
    target[line.start..line.end]
        .iter()
        .take_while(|grapheme| *grapheme == " ")
        .count()
}

fn push_merged(spans: &mut Vec<Span<'static>>, (text, style): (String, Style)) {
    match spans.last_mut() {
        Some(last) if last.style == style => last.content.to_mut().push_str(&text),
        _ => spans.push(Span::styled(text, style)),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use code_racer_engine::{CodeLanguage, SessionOptions, TypingSession};

    use super::*;
    use crate::{
        app::{Disguise, ink::Ink, text_event::TextEvent},
        config::{Look, Theme},
    };

    fn text_of(row: &Row) -> String {
        row.spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn view(session: &TypingSession, syntax: Option<CodeLanguage>) -> SessionView<'_> {
        SessionView {
            session,
            syntax,
            attribution: None,
            stopped_at: None,
            ink: None,
            disguise: None,
        }
    }

    fn still() -> Moment {
        Moment::still(Instant::now())
    }

    #[test]
    fn prose_rows_are_all_numbered() {
        let session = TypingSession::new("one two three four five six", SessionOptions::default());
        let layout = layout(
            &view(&session, None),
            10,
            &Palette::of(Theme::Editor),
            true,
            still(),
        );
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
            still(),
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
        let layout = layout(&view(&session, None), 20, &palette, true, still());
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
            still(),
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
            still(),
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
            text_of(&layout(&view(&session, None), 20, &palette, false, still()).rows[0]),
            "ab"
        );
        let active = layout(&view(&session, None), 20, &palette, true, still());
        let row = &active.rows[0];
        assert_eq!(text_of(row), "ab ");
        let end = row.spans.last().expect("the cursor");
        assert_eq!((end.content.as_ref(), end.style), (" ", palette.cursor));
    }

    fn style_at(layout: &TypingLayout, row: usize, column: usize) -> Style {
        let mut start = 0;
        for span in &layout.rows[row].spans {
            let width = span.content.chars().count();
            if column < start + width {
                return span.style;
            }
            start += width;
        }
        panic!("no column {column} in row {row}");
    }

    #[test]
    fn code_still_to_type_shows_its_syntax_dimmed_and_lights_up_once_typed() {
        let mut session = TypingSession::new("let x = 1;\nlet y = 2;", SessionOptions::default());
        let now = Instant::now();
        for ch in "let x = 1;\n".chars() {
            session.type_char(ch, now);
        }
        let palette = Palette::of(Theme::Editor);
        let layout = layout(
            &view(&session, Some(CodeLanguage::Rust)),
            40,
            &palette,
            false,
            still(),
        );
        let ghost = palette.ghost.expect("true colours");
        let typed_keyword = style_at(&layout, 0, 0);
        let pending_keyword = style_at(&layout, 1, 0);
        let pending_number = style_at(&layout, 1, 8);
        assert_eq!(typed_keyword.fg, Some(palette.lit.keyword));
        assert_eq!(pending_keyword.fg, Some(ghost.keyword));
        assert_eq!(pending_number.fg, Some(ghost.number));
        assert_ne!(pending_keyword.fg, typed_keyword.fg);
    }

    #[test]
    fn brackets_take_the_colour_of_their_depth() {
        let session = TypingSession::new("f(a[0])", SessionOptions::default());
        let palette = Palette::of(Theme::Editor);
        let layout = layout(
            &view(&session, Some(CodeLanguage::Rust)),
            40,
            &palette,
            false,
            still(),
        );
        let ghost = palette.ghost.expect("true colours");
        let outer = style_at(&layout, 0, 1);
        let inner = style_at(&layout, 0, 3);
        let inner_close = style_at(&layout, 0, 5);
        let outer_close = style_at(&layout, 0, 6);
        assert_eq!(outer.fg, Some(ghost.bracket(0)));
        assert_eq!(inner.fg, Some(ghost.bracket(1)));
        assert_eq!(inner_close.fg, inner.fg);
        assert_eq!(outer_close.fg, outer.fg);
    }

    #[test]
    fn brackets_in_strings_are_not_counted() {
        let target: Vec<String> = "(\"(\")".chars().map(String::from).collect();
        let tokens = syntax::highlight(&target, CodeLanguage::Rust);
        let depths = bracket_depths(&target, &tokens);
        assert_eq!(depths, [Some(0), None, None, None, Some(0)]);
    }

    #[test]
    fn indentation_shows_guides_at_each_level() {
        let text = "fn a() {\n    if b {\n        c();\n    }\n}";
        let session = TypingSession::new(text, SessionOptions::default());
        let palette = Palette::of(Theme::Editor);
        let layout = layout(
            &view(&session, Some(CodeLanguage::Rust)),
            40,
            &palette,
            false,
            still(),
        );
        assert_eq!(text_of(&layout.rows[1]), "│   if b { ");
        assert_eq!(text_of(&layout.rows[2]), "│   │   c(); ");
        assert_eq!(text_of(&layout.rows[0]), "fn a() { ");
    }

    #[test]
    fn prose_has_no_indentation_guides() {
        let session = TypingSession::new("a    b", SessionOptions::default());
        let layout = layout(
            &view(&session, None),
            40,
            &Palette::of(Theme::Editor),
            false,
            still(),
        );
        assert_eq!(text_of(&layout.rows[0]), "a    b");
    }

    #[test]
    fn fresh_ink_glows_then_dries() {
        let mut session = TypingSession::new("abc", SessionOptions::default());
        let mut ink = Ink::default();
        let now = Instant::now();
        ink.apply(TextEvent::Typed('a'), &mut session, now);
        let palette = Palette::of(Theme::Editor);
        let view = SessionView {
            ink: Some(&ink),
            ..view(&session, None)
        };
        let glowing = Moment {
            trail: false,
            ..Moment::moving(now)
        };
        let fresh = layout(&view, 20, &palette, false, glowing);
        let trailing = layout(&view, 20, &palette, false, Moment::moving(now));
        assert_eq!(
            style_at(&trailing, 0, 0).fg,
            Some(palette.strong),
            "the trail shows fresh text instead of the glow"
        );
        let dry = layout(
            &view,
            20,
            &palette,
            false,
            Moment::moving(now + DRYING_TIME),
        );
        let still = layout(&view, 20, &palette, false, Moment::still(now));
        assert_eq!(style_at(&fresh, 0, 0).fg, palette.glow);
        assert_eq!(style_at(&dry, 0, 0).fg, Some(palette.strong));
        assert_eq!(
            style_at(&still, 0, 0).fg,
            Some(palette.strong),
            "nothing glows without animations"
        );
        assert!(is_moving(&view, Moment::moving(now)));
        assert!(!is_moving(&view, Moment::still(now)));
    }

    #[test]
    fn the_cursor_leaves_a_fading_trail_of_its_colour() {
        let mut session = TypingSession::new("abcdef", SessionOptions::default());
        let mut ink = Ink::default();
        let now = Instant::now();
        for ch in "abc".chars() {
            ink.apply(TextEvent::Typed(ch), &mut session, now);
        }
        let palette = Palette::of(Theme::Editor);
        let view = SessionView {
            ink: Some(&ink),
            ..view(&session, None)
        };
        let trailing = layout(&view, 20, &palette, true, Moment::moving(now));
        let behind = style_at(&trailing, 0, 2).bg;
        assert!(
            behind.is_some() && behind != palette.cursorline.bg,
            "{behind:?}"
        );
        assert_eq!(
            style_at(&trailing, 0, 3),
            palette.cursor,
            "the cursor itself"
        );
        let without = Moment {
            trail: false,
            ..Moment::moving(now)
        };
        let plain = layout(&view, 20, &palette, true, without);
        assert_eq!(style_at(&plain, 0, 2).bg, None, "no trail when it is off");
        let mono = layout(
            &view,
            20,
            &Palette::of(Theme::Mono),
            true,
            Moment::moving(now),
        );
        assert_eq!(
            style_at(&mono, 0, 2).bg,
            None,
            "nor where colours cannot fade"
        );
    }

    #[test]
    fn the_cursor_breathes_only_once_the_player_pauses() {
        let mut session = TypingSession::new("abc", SessionOptions::default());
        let mut ink = Ink::default();
        let now = Instant::now();
        ink.apply(TextEvent::Typed('a'), &mut session, now);
        let palette = Palette::of(Theme::Editor);
        let view = SessionView {
            ink: Some(&ink),
            ..view(&session, None)
        };
        let typing = cursor_style(&view, &palette, Moment::moving(now));
        assert_eq!(typing, palette.cursor);
        let half_breath = BREATHE_AFTER + BREATH / 2;
        let pausing = cursor_style(&view, &palette, Moment::moving(now + half_breath));
        assert_ne!(pausing.bg, palette.cursor.bg);
        let mono = Palette::of(Theme::Mono);
        assert_eq!(
            cursor_style(&view, &mono, Moment::moving(now + half_breath)),
            mono.cursor
        );
    }

    #[test]
    fn a_look_dresses_prose_and_numbers_its_lines() {
        let session = TypingSession::new("ship the release notes", SessionOptions::default());
        let palette = Palette::of(Theme::Editor);
        let disguise = Disguise {
            look: Look::Todo,
            ..Disguise::default()
        };
        let view = SessionView {
            disguise: Some(disguise),
            ..view(&session, None)
        };
        let layout = layout(&view, 20, &palette, true, still());
        assert_eq!(text_of(&layout.rows[0]), "# TODO");
        assert!(text_of(&layout.rows[2]).starts_with("- [ ] ship"));
        assert_eq!(layout.cursor_row, 2, "the header comes first");
        let numbers: Vec<Option<usize>> = layout.rows.iter().map(|row| row.number).collect();
        assert_eq!(numbers[..3], [Some(1), Some(2), Some(3)]);
        assert!(
            layout
                .rows
                .iter()
                .skip(2)
                .all(|row| text_of(row).width() <= 20),
            "the checkboxes take room from the text"
        );
    }

    #[test]
    fn code_ignores_the_look() {
        let session = TypingSession::new("let a = 1;", SessionOptions::default());
        let view = SessionView {
            disguise: Some(Disguise {
                look: Look::Todo,
                ..Disguise::default()
            }),
            ..view(&session, Some(CodeLanguage::Rust))
        };
        let layout = layout(&view, 40, &Palette::of(Theme::Editor), false, still());
        assert_eq!(layout.rows.len(), 1);
        assert_eq!(text_of(&layout.rows[0]), "let a = 1;");
    }
}
