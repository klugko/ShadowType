use ratatui::{
    style::{Color, Style},
    text::Span,
};

use super::{
    HEIGHT, panel,
    pose::{Eyes, Pose},
};
use crate::ui::theme::Palette;

/// Pixels of the canvas: the ghost, and room for it to bob and shake.
pub(super) const CANVAS_WIDTH: usize = 14;
const CANVAS_HEIGHT: usize = 2 * HEIGHT as usize;

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
pub(super) enum Pixel {
    Body,
    Shade,
    Eye,
    Shine,
    Blush,
}

pub(super) type Canvas = [[Option<Pixel>; CANVAS_WIDTH]; CANVAS_HEIGHT];

/// A pixel of the face: its column and row on the body, and what it shows.
type Dot = (usize, usize, Pixel);

pub(super) fn canvas(pose: Pose) -> Canvas {
    let mut canvas = [[None; CANVAS_WIDTH]; CANVAS_HEIGHT];
    let top = 2 - pose.lift.min(2);
    let left = usize::try_from(1 + pose.shake).unwrap_or(1);
    let rows = BODY.iter().chain([&FRINGES[pose.fringe % FRINGES.len()]]);
    for (y, row) in rows.enumerate() {
        let shade = row.rfind('#');
        for (x, pixel) in row.char_indices() {
            if pixel == '#' {
                let shaded = (3..=10).contains(&y) && Some(x) == shade;
                canvas[top + y][left + x] = Some(if shaded { Pixel::Shade } else { Pixel::Body });
            }
        }
    }
    for (x, y, pixel) in face(pose) {
        canvas[top + y][left + x + pose.look] = Some(pixel);
    }
    canvas
}

fn face(pose: Pose) -> Vec<Dot> {
    let mut dots = eye(pose.eyes, 3);
    dots.extend(eye(pose.eyes, 7));
    dots.extend([1, 2, 9, 10].map(|x| (x, 6, Pixel::Blush)));
    dots.extend([(5, 7, Pixel::Eye), (6, 7, Pixel::Eye)]);
    if pose.open_mouth {
        dots.extend([(5, 8, Pixel::Eye), (6, 8, Pixel::Eye)]);
    }
    dots
}

/// The eye whose left half is at column `x`.
fn eye(eyes: Eyes, x: usize) -> Vec<Dot> {
    match eyes {
        Eyes::Open => vec![
            (x, 4, Pixel::Shine),
            (x + 1, 4, Pixel::Eye),
            (x, 5, Pixel::Eye),
            (x + 1, 5, Pixel::Eye),
        ],
        Eyes::Closed => vec![(x, 5, Pixel::Eye), (x + 1, 5, Pixel::Eye)],
        Eyes::Happy => vec![
            (x, 4, Pixel::Eye),
            (x + 1, 4, Pixel::Eye),
            (x - 1, 5, Pixel::Eye),
            (x + 2, 5, Pixel::Eye),
        ],
        Eyes::Wide => vec![
            (x, 3, Pixel::Shine),
            (x + 1, 3, Pixel::Eye),
            (x, 4, Pixel::Eye),
            (x + 1, 4, Pixel::Eye),
            (x, 5, Pixel::Eye),
            (x + 1, 5, Pixel::Eye),
        ],
    }
}

/// The terminal cells of row `row` of the mascot, two rows of `canvas` each.
pub(super) fn cells(
    canvas: &Canvas,
    row: usize,
    palette: &Palette,
) -> impl Iterator<Item = Span<'static>> {
    (0..CANVAS_WIDTH).map(move |x| half_block(canvas[2 * row][x], canvas[2 * row + 1][x], palette))
}

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

/**
 * The colour of `pixel`, `None` where the background shows through: the
 * eyes of a colourless ghost are holes in it.
 */
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        app::mascot::Mood,
        ui::mascot::{HEIGHT, STILL},
    };

    const MOODS: [Mood; 6] = [
        Mood::Idle,
        Mood::Typing,
        Mood::Oops,
        Mood::Happy,
        Mood::Proud,
        Mood::Asleep,
    ];

    /// The mascot drawn as text, one character per pixel pair.
    fn sketch(mood: Mood, age: Duration) -> Vec<String> {
        let palette = Palette::of(crate::config::Theme::Mono);
        let canvas = canvas(Pose::of(mood, age));
        (0..usize::from(HEIGHT))
            .map(|row| {
                cells(&canvas, row, &palette)
                    .map(|cell| cell.content.into_owned())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn every_pose_fits_the_canvas() {
        for mood in MOODS {
            for millis in (0..4_000).step_by(37) {
                canvas(Pose::of(mood, Duration::from_millis(millis)));
            }
        }
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
