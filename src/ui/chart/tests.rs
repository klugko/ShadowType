use rand::{RngExt, SeedableRng, rngs::StdRng};
use unicode_width::UnicodeWidthStr;

use super::*;

fn labels(lines: &[String]) -> Vec<i64> {
    lines
        .iter()
        .filter_map(|line| line.split([TICK, BASELINE_TICK]).next())
        .map(|label| label.trim().parse().expect("integer label"))
        .collect()
}

fn plot(line: &str) -> &str {
    line.split_once([TICK, BASELINE_TICK])
        .map_or(line, |(_, plot)| plot)
}

#[test]
fn nothing_to_plot_renders_no_rows() {
    assert!(line_chart(&[], 40, 8).is_empty());
    assert!(line_chart(&[f64::NAN, f64::INFINITY], 40, 8).is_empty());
    assert!(line_chart(&[1.0, 2.0], 40, 0).is_empty());
}

#[test]
fn rows_have_the_requested_height_and_fill_the_width() {
    for len in [1, 3, 300] {
        let series: Vec<f64> = (0..len).map(|step| f64::from(step % 37) * 3.5).collect();
        for height in 1..=12 {
            for width in 0..=50 {
                let lines = line_chart(&series, width, height);
                assert_eq!(lines.len(), usize::from(height), "{width}x{height}");
                for line in &lines {
                    assert_eq!(
                        line.width(),
                        usize::from(width),
                        "{len} values at {width}x{height}: {line:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn rising_and_falling_steps_use_rounded_corners() {
    assert_eq!(line_chart(&[0.0, 10.0], 6, 2), ["10 ┤ ╭", " 0 ┼─╯"]);
    assert_eq!(line_chart(&[10.0, 0.0], 6, 2), ["10 ┤─╮", " 0 ┼ ╰"]);
}

#[test]
fn steep_steps_are_joined_by_vertical_bars() {
    let lines = line_chart(&[0.0, 30.0, 0.0], 7, 4);
    let plots: Vec<&str> = lines.iter().map(|line| plot(line)).collect();
    assert_eq!(plots, [" ╭╮", " ││", " ││", "─╯╰"]);
}

#[test]
fn short_series_are_stretched_across_the_width() {
    assert_eq!(
        line_chart(&[0.0, 30.0], 10, 4),
        ["30 ┤     ╭", "20 ┤   ╭─╯", "10 ┤ ╭─╯  ", " 0 ┼─╯    "]
    );
}

#[test]
fn interpolation_keeps_the_values_and_runs_straight_between_them() {
    assert_eq!(
        interpolate(&[10.0, 20.0], 5),
        [10.0, 12.5, 15.0, 17.5, 20.0]
    );
    assert_eq!(interpolate(&[1.0, 3.0, 2.0], 5), [1.0, 2.0, 3.0, 2.5, 2.0]);
    assert_eq!(interpolate(&[4.0], 3), [4.0; 3]);
    assert!(interpolate(&[], 3).is_empty());
}

#[test]
fn labels_span_the_series_minimum_and_maximum() {
    let lines = line_chart(&[40.0, 55.0, 62.0, 58.0, 71.0, 80.0, 77.0, 85.4], 40, 8);
    let labels = labels(&lines);
    assert_eq!(labels.first(), Some(&85));
    assert_eq!(labels.last(), Some(&40));
    assert!(labels.windows(2).all(|pair| pair[0] > pair[1]));
    assert!(lines[7].starts_with("40 ┼"));
    assert!(lines[..7].iter().all(|line| line.contains(TICK)));
}

#[test]
fn narrow_ranges_get_one_distinct_label_per_row() {
    let lines = line_chart(&[50.2, 51.4, 50.9], 40, 6);
    let labels = labels(&lines);
    assert_eq!(labels, [53, 52, 51, 50, 49, 48]);
    let line_row = lines.iter().position(|line| plot(line).contains('─'));
    assert!(line_row.is_some());
}

#[test]
fn flat_series_is_a_straight_line_in_the_middle() {
    let lines = line_chart(&[60.0; 5], 40, 5);
    assert_eq!(labels(&lines), [62, 61, 60, 59, 58]);
    assert_eq!(plot(&lines[2]), "─".repeat(36));
    assert!(
        lines
            .iter()
            .enumerate()
            .all(|(row, line)| row == 2 || plot(line).trim().is_empty())
    );
}

#[test]
fn single_value_is_a_flat_line_across_the_width() {
    let lines = line_chart(&[72.0], 7, 3);
    assert_eq!(lines, ["73 ┤   ", "72 ┤───", "71 ┼   "]);
}

#[test]
fn labels_never_go_below_zero_for_positive_series() {
    let lines = line_chart(&[0.0, 1.0], 20, 6);
    assert_eq!(labels(&lines), [5, 4, 3, 2, 1, 0]);
}

#[test]
fn negative_labels_are_right_aligned() {
    let lines = line_chart(&[-120.0, 5.0], 8, 2);
    assert_eq!(lines, ["   5 ┤ ╭", "-120 ┼─╯"]);
}

#[test]
fn extra_points_are_averaged_into_columns() {
    let dense = [0.0, 0.0, 30.0, 30.0, 0.0, 0.0, 30.0, 30.0];
    let sparse = [0.0, 30.0, 0.0, 30.0];
    let width = 4 + 4;
    assert_eq!(line_chart(&dense, width, 4), line_chart(&sparse, width, 4));
}

#[test]
fn averaged_columns_keep_the_full_series_range_on_the_axis() {
    let lines = line_chart(&[0.0, 100.0, 0.0, 100.0], 6, 3);
    assert_eq!(labels(&lines), [100, 50, 0]);
    assert_eq!(
        lines.iter().map(|line| plot(line)).collect::<Vec<_>>(),
        [" ", "─", " "]
    );
}

#[test]
fn labels_are_dropped_when_they_leave_no_room_for_the_curve() {
    assert_eq!(line_chart(&[100.0, 250.0], 4, 2), ["  ╭─", "──╯ "]);
    assert_eq!(line_chart(&[100.0, 250.0], 0, 2), ["", ""]);
}

#[test]
fn non_finite_values_are_skipped() {
    let with_gaps = line_chart(&[10.0, f64::NAN, 20.0, f64::NEG_INFINITY], 20, 3);
    assert_eq!(with_gaps, line_chart(&[10.0, 20.0], 20, 3));
}

fn random_series(rng: &mut StdRng) -> Vec<f64> {
    let len = rng.random_range(0..120);
    let (low, high) = [(0, 200), (-500, 500), (40, 43), (7, 8), (-3, 0)][rng.random_range(0..5)];
    (0..len)
        .map(|_| f64::from(rng.random_range(low..high)) + rng.random_range(0..4) as f64 / 4.0)
        .collect()
}

fn columns(lines: &[String]) -> Vec<Vec<char>> {
    let plots: Vec<Vec<char>> = lines
        .iter()
        .map(|line| plot(line).chars().collect())
        .collect();
    let width = plots.first().map_or(0, Vec::len);
    (0..width)
        .map(|column| plots.iter().map(|row| row[column]).collect())
        .collect()
}

fn assert_curve_is_connected(lines: &[String], context: &str) {
    for (index, column) in columns(lines).iter().enumerate() {
        let drawn: Vec<usize> = (0..column.len())
            .filter(|&row| column[row] != ' ')
            .collect();
        let (Some(first), Some(last)) = (drawn.first(), drawn.last()) else {
            panic!("{context}: column {index} is empty");
        };
        assert_eq!(
            last - first + 1,
            drawn.len(),
            "{context}: column {index} has a gap"
        );
        if index == 0 {
            assert_eq!(drawn.len(), 1, "{context}: first column");
            assert_eq!(column[*first], '─', "{context}: first column");
        }
    }
}

fn assert_labels_span_the_series(values: &[f64], lines: &[String], context: &str) {
    let labelled = lines
        .iter()
        .all(|line| line.contains([TICK, BASELINE_TICK]));
    if !labelled || lines.len() < 2 {
        return;
    }
    let labels = labels(lines);
    let min = values.iter().copied().fold(f64::INFINITY, f64::min).round() as i64;
    let max = values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .round() as i64;
    assert!(
        labels.windows(2).all(|pair| pair[0] > pair[1]),
        "{context}: {labels:?}"
    );
    assert!(
        labels.first().is_some_and(|top| *top >= max),
        "{context}: {labels:?}"
    );
    assert!(
        labels.last().is_some_and(|bottom| *bottom <= min),
        "{context}: {labels:?}"
    );
}

#[test]
fn random_charts_have_the_requested_size_and_a_connected_curve() {
    let mut rng = StdRng::seed_from_u64(17);
    for _ in 0..3_000 {
        let values = random_series(&mut rng);
        let (width, height) = (rng.random_range(0..70), rng.random_range(0..14));
        let lines = line_chart(&values, width, height);
        let context = format!("{values:?} at {width}x{height}");
        let rows = if values.is_empty() { 0 } else { height };
        assert_eq!(lines.len(), usize::from(rows), "{context}");
        assert!(
            lines.iter().all(|line| line.width() == usize::from(width)),
            "{context}"
        );
        if lines.is_empty() || width == 0 {
            continue;
        }
        assert_curve_is_connected(&lines, &context);
        assert_labels_span_the_series(&values, &lines, &context);
    }
}

#[test]
fn repeated_points_average_back_to_the_original_curve() {
    let mut rng = StdRng::seed_from_u64(23);
    for _ in 0..500 {
        let values: Vec<f64> = (0..rng.random_range(1..30))
            .map(|_| f64::from(rng.random_range(0..150)))
            .collect();
        let repeat = rng.random_range(2..5);
        let dense: Vec<f64> = values
            .iter()
            .flat_map(|value| std::iter::repeat_n(*value, repeat))
            .collect();
        let height = rng.random_range(1..10);
        let wide = line_chart(&values, 200, height);
        let axis = wide[0].width() - plot(&wide[0]).chars().count();
        let width = u16::try_from(axis + values.len()).expect("narrow chart");
        assert_eq!(
            line_chart(&dense, width, height),
            line_chart(&values, width, height),
            "{values:?} x{repeat}"
        );
    }
}

#[test]
fn non_finite_values_never_change_the_chart() {
    let mut rng = StdRng::seed_from_u64(29);
    for _ in 0..500 {
        let values = random_series(&mut rng);
        let mut noisy = values.clone();
        for _ in 0..rng.random_range(1..4) {
            let junk = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY][rng.random_range(0..3)];
            noisy.insert(rng.random_range(0..=noisy.len()), junk);
        }
        let (width, height) = (rng.random_range(0..60), rng.random_range(0..10));
        assert_eq!(
            line_chart(&noisy, width, height),
            line_chart(&values, width, height)
        );
    }
}
