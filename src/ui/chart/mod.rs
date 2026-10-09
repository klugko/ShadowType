//! Line charts drawn with box-drawing characters, in the style of asciichart.

use std::cmp::Ordering;

const BASELINE_TICK: char = '┼';
const TICK: char = '┤';
const TICK_COLUMN_WIDTH: usize = 2;

/**
 * Renders `values` as a line chart of exactly `height` rows (none when there is
 * nothing to plot), each exactly `width` cells wide. Non-finite values are
 * ignored.
 *
 * Every row starts with a right-aligned integer label and an axis tick (`┼` on
 * the bottom row, `┤` above it). More values than columns are averaged into a
 * column each; fewer are joined by straight lines. The axis spans the minimum
 * and maximum of the series, but labels never repeat: a series that spans fewer
 * units than there are rows gets one unit per row, centred on the data. The
 * labels are dropped when they leave no room for at least one column.
 */
pub fn line_chart(values: &[f64], width: u16, height: u16) -> Vec<String> {
    let finite: Vec<f64> = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    if finite.is_empty() || height == 0 {
        return Vec::new();
    }
    let (min, max) = finite
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(*value), max.max(*value))
        });
    let scale = Scale::fit(min, max, usize::from(height));
    let width = usize::from(width);
    let label_width = Some(scale.label_width()).filter(|label| label + TICK_COLUMN_WIDTH < width);
    let plot_width = width - label_width.map_or(0, |label| label + TICK_COLUMN_WIDTH);
    let levels: Vec<usize> = resample(&finite, plot_width)
        .into_iter()
        .map(|value| scale.level(value))
        .collect();
    (0..=scale.top_level)
        .rev()
        .map(|level| render_row(level, scale, label_width, &levels))
        .collect()
}

/// The series spread over exactly `columns` columns.
fn resample(values: &[f64], columns: usize) -> Vec<f64> {
    match columns {
        0 => Vec::new(),
        _ if values.len() >= columns => average_into(values, columns),
        _ => interpolate(values, columns),
    }
}

/**
 * Consecutive values averaged into `columns` columns, at least one value
 * each since there are at least as many values as columns.
 */
fn average_into(values: &[f64], columns: usize) -> Vec<f64> {
    let mut buckets = vec![(0.0, 0.0); columns];
    for (index, value) in values.iter().enumerate() {
        let (sum, members) = &mut buckets[index * columns / values.len()];
        *sum += value;
        *members += 1.0;
    }
    buckets
        .into_iter()
        .map(|(sum, members)| sum / members)
        .collect()
}

/**
 * `columns` values running straight from each value to the next, the
 * first and last ones unchanged.
 */
fn interpolate(values: &[f64], columns: usize) -> Vec<f64> {
    let Some(last) = values.len().checked_sub(1) else {
        return Vec::new();
    };
    let steps = columns.saturating_sub(1).max(1) as f64;
    (0..columns)
        .map(|column| {
            let position = (column * last) as f64 / steps;
            let index = (position.floor() as usize).min(last);
            let next = (index + 1).min(last);
            let fraction = position - index as f64;
            values[index] + (values[next] - values[index]) * fraction
        })
        .collect()
}

/// Maps values to rows: row `0` is the bottom of the chart.
#[derive(Debug, Clone, Copy)]
struct Scale {
    low: f64,
    high: f64,
    top_level: usize,
}

impl Scale {
    fn fit(min: f64, max: f64, rows: usize) -> Self {
        let top_level = rows.saturating_sub(1);
        let intervals = top_level as f64;
        if top_level > 0 && max - min >= intervals {
            return Self {
                low: min,
                high: max,
                top_level,
            };
        }
        let spread = max.round() - min.round();
        let slack = (intervals - spread).max(0.0);
        let centred = min.round() - (slack / 2.0).floor();
        let low = if min.round() >= 0.0 {
            centred.max(0.0)
        } else {
            centred
        };
        Self {
            low,
            high: low + intervals,
            top_level,
        }
    }

    fn level(self, value: f64) -> usize {
        if self.top_level == 0 {
            return 0;
        }
        let position = (value - self.low) / (self.high - self.low) * self.top_level as f64;
        (position.round().max(0.0) as usize).min(self.top_level)
    }

    fn label(self, level: usize) -> i64 {
        if self.top_level == 0 {
            return self.low.round() as i64;
        }
        let ratio = level as f64 / self.top_level as f64;
        (self.low * (1.0 - ratio) + self.high * ratio).round() as i64
    }

    fn label_width(self) -> usize {
        display_width(self.label(0)).max(display_width(self.label(self.top_level)))
    }
}

fn display_width(number: i64) -> usize {
    let digits = number
        .unsigned_abs()
        .checked_ilog10()
        .map_or(1, |exponent| exponent as usize + 1);
    digits + usize::from(number < 0)
}

fn render_row(level: usize, scale: Scale, label_width: Option<usize>, levels: &[usize]) -> String {
    let mut row = match label_width {
        Some(width) => {
            let tick = if level == 0 { BASELINE_TICK } else { TICK };
            format!("{:>width$} {tick}", scale.label(level))
        }
        None => String::new(),
    };
    row.reserve(levels.len() * '─'.len_utf8());
    let mut previous = levels.first().copied().unwrap_or_default();
    for &current in levels {
        row.push(segment(previous, current, level));
        previous = current;
    }
    row
}

/**
 * The glyph at row `level` of a column where the curve enters at level `from`
 * on the left and leaves at level `to` on the right.
 */
fn segment(from: usize, to: usize, level: usize) -> char {
    if level < from.min(to) || level > from.max(to) {
        return ' ';
    }
    match (from.cmp(&to), level == from, level == to) {
        (Ordering::Equal, _, _) => '─',
        (Ordering::Less, true, _) => '╯',
        (Ordering::Less, _, true) => '╭',
        (Ordering::Greater, true, _) => '╮',
        (Ordering::Greater, _, true) => '╰',
        _ => '│',
    }
}

#[cfg(test)]
mod tests;
