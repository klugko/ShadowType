//! The mascot: a little pixel-art ghost that lives under the explorer. It
//! floats about, types along, startles at mistakes, rejoices at the end of
//! a text and naps when nothing happens.
//!
//! It is drawn with half blocks, two pixels to a cell: `▀` takes the colour
//! of its upper pixel as foreground and of its lower pixel as background.

use std::time::Duration;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app::mascot::Mood, ui::theme::Palette};

/// Rows the mascot takes.
pub const HEIGHT: u16 = 7;
/// Columns the mascot takes, the sparkles around it included.
pub const WIDTH: u16 = 20;
/// Column of the sprite in the area of the mascot.
const LEFT: usize = 2;
/// Pixels of the canvas: the ghost, and room for it to bob and shake.
const CANVAS_WIDTH: usize = 14;
const CANVAS_HEIGHT: usize = 2 * HEIGHT as usize;
/// The age the mascot is drawn at when nothing moves: its eyes open.
pub const STILL: Duration = Duration::from_secs(1);
/// How often the mascot blinks, and for how long.
const BLINK_EVERY: Duration = Duration::from_millis(3_700);
const BLINK: Duration = Duration::from_millis(170);

/// The ghost, 12 pixels wide: `#` is its body, `.` the background.
const BODY: [&str; 11] = [
    "....####....",
    "..########..",
    ".##########.",
    ".##########.",
    "############",
    "############",
    "############",
    "############",
    "############",
    "############",
    "############",
];
/// The fringe at the bottom of the ghost, which waves as it floats.
const FRINGES: [&str; 2] = [".##..##..##.", "#..##..##..#"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pixel {
    Body,
    Shade,
    Eye,
    Shine,
    Blush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Eyes {
    Open,
    Closed,
    Happy,
    Wide,
}

/// One pose of the ghost, from its mood at some age.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pose {
    /// How far above its lowest point it floats, in pixels.
    lift: usize,
    /// How far right it shakes, from -1 to 1 pixels.
    shake: isize,
    /// How far right it looks.
    look: usize,
    eyes: Eyes,
    fringe: usize,
    open_mouth: bool,
}

impl Pose {
    fn of(mood: Mood, age: Duration) -> Self {
        let millis = age.as_millis();
        let step = |period: u128, steps: u128| (millis % period) * steps / period;
        let blinking = millis % BLINK_EVERY.as_millis() < BLINK.as_millis();
        let open = if blinking { Eyes::Closed } else { Eyes::Open };
        let calm = Self {
            lift: 1,
            shake: 0,
            look: 0,
            eyes: open,
            fringe: 0,
            open_mouth: false,
        };
        match mood {
            Mood::Idle => Self {
                lift: usize::from(step(1_800, 2) == 0),
                fringe: usize::try_from(step(900, 2)).unwrap_or(0),
                ..calm
            },
            Mood::Typing => Self {
                lift: usize::from(step(700, 2) == 0),
                look: 1,
                fringe: usize::try_from(step(400, 2)).unwrap_or(0),
                ..calm
            },
            Mood::Oops => Self {
                lift: 2,
                shake: if step(160, 2) == 0 { -1 } else { 1 },
                eyes: Eyes::Wide,
                open_mouth: true,
                ..calm
            },
            Mood::Happy | Mood::Proud => {
                let hop = [0, 1, 2, 1];
                let period = if mood == Mood::Proud { 480 } else { 720 };
                Self {
                    lift: hop[usize::try_from(step(period, 4)).unwrap_or(0)],
                    eyes: Eyes::Happy,
                    fringe: usize::try_from(step(period / 2, 2)).unwrap_or(0),
                    open_mouth: true,
                    ..calm
                }
            }
            Mood::Asleep => Self {
                lift: 0,
                eyes: Eyes::Closed,
                ..calm
            },
        }
    }

    /// The pixels of the ghost in this pose.
    fn canvas(self) -> [[Option<Pixel>; CANVAS_WIDTH]; CANVAS_HEIGHT] {
        let mut canvas = [[None; CANVAS_WIDTH]; CANVAS_HEIGHT];
        let top = 2 - self.lift.min(2);
        let left = usize::try_from(1 + self.shake).unwrap_or(1);
        let rows = BODY.iter().chain([&FRINGES[self.fringe % FRINGES.len()]]);
        for (y, row) in rows.enumerate() {
            let shade = row.rfind('#');
            for (x, pixel) in row.char_indices() {
                if pixel == '#' {
                    let shaded = (3..=10).contains(&y) && Some(x) == shade;
                    canvas[top + y][left + x] =
                        Some(if shaded { Pixel::Shade } else { Pixel::Body });
                }
            }
        }
        let mut paint = |x: usize, y: usize, pixel: Pixel| {
            canvas[top + y][left + x + self.look] = Some(pixel);
        };
        for eye in [3, 7] {
            match self.eyes {
                Eyes::Open => {
                    paint(eye, 4, Pixel::Shine);
                    paint(eye + 1, 4, Pixel::Eye);
                    paint(eye, 5, Pixel::Eye);
                    paint(eye + 1, 5, Pixel::Eye);
                }
                Eyes::Closed => {
                    paint(eye, 5, Pixel::Eye);
                    paint(eye + 1, 5, Pixel::Eye);
                }
                Eyes::Happy => {
                    paint(eye, 4, Pixel::Eye);
                    paint(eye + 1, 4, Pixel::Eye);
                    paint(eye - 1, 5, Pixel::Eye);
                    paint(eye + 2, 5, Pixel::Eye);
                }
                Eyes::Wide => {
                    for y in 3..=5 {
                        paint(eye, y, Pixel::Eye);
                        paint(eye + 1, y, Pixel::Eye);
                    }
                    paint(eye, 3, Pixel::Shine);
                }
            }
        }
        for x in [1, 2, 9, 10] {
            paint(x, 6, Pixel::Blush);
        }
        paint(5, 7, Pixel::Eye);
        paint(6, 7, Pixel::Eye);
        if self.open_mouth {
            paint(5, 8, Pixel::Eye);
            paint(6, 8, Pixel::Eye);
        }
        canvas
    }
}

/// Draws the mascot feeling `mood` at `age` in `area`. Without animations
/// it holds its first pose.
pub fn render(frame: &mut Frame, area: Rect, mood: Mood, age: Duration, palette: &Palette) {
    let pose = Pose::of(mood, age);
    let canvas = pose.canvas();
    let mut lines: Vec<Vec<Span<'static>>> = (0..HEIGHT)
        .map(|row| {
            let row = usize::from(row);
            let mut spans: Vec<Span<'static>> = (0..LEFT)
                .map(|_| Span::styled(" ", panel(palette)))
                .collect();
            spans.extend(
                (0..CANVAS_WIDTH)
                    .map(|x| half_block(canvas[2 * row][x], canvas[2 * row + 1][x], palette)),
            );
            spans
        })
        .collect();
    for (column, row, text, color) in decorations(mood, age, palette) {
        if let Some(line) = lines.get_mut(row) {
            put(line, LEFT + CANVAS_WIDTH + column, text, palette.fg(color));
        }
    }
    let lines: Vec<Line> = lines.into_iter().map(Line::from).collect();
    frame.render_widget(Paragraph::new(lines).style(panel(palette)), area);
}

/// Sparkles, sighs and snores around the ghost: column after the canvas,
/// row, text and colour.
fn decorations(
    mood: Mood,
    age: Duration,
    palette: &Palette,
) -> Vec<(usize, usize, &'static str, Color)> {
    let beat = (age.as_millis() / 300).is_multiple_of(2);
    match mood {
        Mood::Asleep => vec![
            (0, 3, "z", palette.muted),
            (1, 2, "z", palette.muted),
            (2, 1, "Z", palette.text),
        ],
        Mood::Oops => vec![(0, 1, "!", palette.warning)],
        Mood::Typing => {
            let dots =
                ["·  ", "·· ", "···"][usize::try_from(age.as_millis() / 400 % 3).unwrap_or(0)];
            vec![(0, 4, dots, palette.muted)]
        }
        Mood::Happy if beat => vec![(0, 1, "✦", palette.warning), (2, 3, "·", palette.accent)],
        Mood::Happy => vec![(1, 0, "·", palette.warning), (0, 3, "✧", palette.accent)],
        Mood::Proud if beat => vec![
            (0, 1, "✦", palette.warning),
            (2, 3, "✧", palette.accent),
            (1, 5, "·", palette.success),
        ],
        Mood::Proud => vec![
            (1, 0, "✧", palette.warning),
            (0, 3, "✦", palette.accent),
            (2, 4, "✦", palette.success),
        ],
        Mood::Idle => Vec::new(),
    }
}

/// Writes `text` at `column` of a row of one-column spans.
fn put(line: &mut Vec<Span<'static>>, column: usize, text: &str, style: Style) {
    while line.len() < column {
        line.push(Span::raw(" "));
    }
    let span = Span::styled(text.to_owned(), style);
    if column < line.len() {
        line[column] = span;
    } else {
        line.push(span);
    }
}

fn panel(palette: &Palette) -> Style {
    Style::new().bg(palette.panel)
}

/// The cell for an upper and a lower pixel.
fn half_block(top: Option<Pixel>, bottom: Option<Pixel>, palette: &Palette) -> Span<'static> {
    let top = top.and_then(|pixel| color(pixel, palette));
    let bottom = bottom.and_then(|pixel| color(pixel, palette));
    let panel = panel(palette);
    match (top, bottom) {
        (None, None) => Span::styled(" ", panel),
        (Some(top), None) => Span::styled("▀", panel.fg(top)),
        (None, Some(bottom)) => Span::styled("▄", panel.fg(bottom)),
        (Some(top), Some(bottom)) if palette.mono || top == bottom => {
            Span::styled("█", panel.fg(top))
        }
        (Some(top), Some(bottom)) => Span::styled("▀", Style::new().fg(top).bg(bottom)),
    }
}

/// The colour of `pixel`, `None` where the background shows through: the
/// eyes of a colourless ghost are holes in it.
fn color(pixel: Pixel, palette: &Palette) -> Option<Color> {
    if palette.mono {
        return match pixel {
            Pixel::Eye | Pixel::Shine => None,
            Pixel::Body | Pixel::Shade | Pixel::Blush => Some(Color::Reset),
        };
    }
    let true_colours = palette.blends();
    Some(match (pixel, true_colours) {
        (Pixel::Body, true) => Color::Rgb(226, 230, 245),
        (Pixel::Shade, true) => Color::Rgb(178, 186, 214),
        (Pixel::Eye, true) => Color::Rgb(35, 38, 52),
        (Pixel::Shine, true) => Color::Rgb(255, 255, 255),
        (Pixel::Blush, true) => Color::Rgb(244, 154, 178),
        (Pixel::Body | Pixel::Shine, false) => Color::White,
        (Pixel::Shade, false) => Color::Gray,
        (Pixel::Eye, false) => Color::Black,
        (Pixel::Blush, false) => Color::LightRed,
    })
}

/// The mascot drawn as text, one character per pixel pair, for tests.
#[cfg(test)]
fn sketch(mood: Mood, age: Duration) -> Vec<String> {
    let palette = Palette::of(crate::config::Theme::Mono);
    let canvas = Pose::of(mood, age).canvas();
    (0..usize::from(HEIGHT))
        .map(|row| {
            (0..CANVAS_WIDTH)
                .map(|x| {
                    half_block(canvas[2 * row][x], canvas[2 * row + 1][x], &palette)
                        .content
                        .into_owned()
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOODS: [Mood; 6] = [
        Mood::Idle,
        Mood::Typing,
        Mood::Oops,
        Mood::Happy,
        Mood::Proud,
        Mood::Asleep,
    ];

    #[test]
    fn every_pose_fits_the_canvas() {
        for mood in MOODS {
            for millis in (0..4_000).step_by(37) {
                Pose::of(mood, Duration::from_millis(millis)).canvas();
            }
        }
    }

    #[test]
    fn the_ghost_floats_while_awake_and_lies_still_asleep() {
        let poses = |mood| -> Vec<Pose> {
            (0..40)
                .map(|step| Pose::of(mood, Duration::from_millis(step * 100)))
                .collect()
        };
        let idle = poses(Mood::Idle);
        assert!(idle.iter().any(|pose| pose.lift != idle[0].lift), "it bobs");
        let asleep = poses(Mood::Asleep);
        assert!(asleep.iter().all(|pose| *pose == asleep[0]), "it rests");
        assert_eq!(asleep[0].eyes, Eyes::Closed);
    }

    #[test]
    fn the_ghost_blinks_now_and_then() {
        assert_eq!(Pose::of(Mood::Idle, Duration::ZERO).eyes, Eyes::Closed);
        assert_eq!(Pose::of(Mood::Idle, BLINK).eyes, Eyes::Open);
        assert_eq!(Pose::of(Mood::Idle, BLINK_EVERY).eyes, Eyes::Closed);
    }

    #[test]
    fn a_colourless_ghost_shows_its_eyes_as_holes() {
        let sketch = sketch(Mood::Idle, STILL);
        let face = sketch.iter().find(|row| {
            let body = row.trim();
            body.starts_with('█') && body.contains(['▀', '▄'])
        });
        assert!(face.is_some(), "{sketch:#?}");
        assert_eq!(sketch.len(), usize::from(HEIGHT));
        assert!(sketch.iter().all(|row| row.chars().count() == CANVAS_WIDTH));
    }
}
