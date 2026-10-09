use super::*;

#[test]
fn wpm_uses_five_characters_per_word() {
    assert_eq!(words_per_minute(300, Duration::from_secs(60)), 60.0);
    assert_eq!(words_per_minute(50, Duration::from_secs(10)), 60.0);
}

#[test]
fn wpm_is_zero_without_elapsed_time() {
    assert_eq!(words_per_minute(42, Duration::ZERO), 0.0);
}

#[test]
fn percentage_defaults_to_perfect() {
    assert_eq!(percentage(0, 0), 100.0);
    assert_eq!(percentage(3, 4), 75.0);
}

#[test]
fn indentation_counts_for_neither_speed_nor_accuracy() {
    let tally = Tally {
        typed: 30,
        correct: 28,
        indentation: 8,
        keystrokes: 25,
        errors: 5,
    };
    let minute = Duration::from_secs(60);
    assert_eq!(tally.correctly_typed(), 20);
    assert_eq!(tally.wpm(minute), 4.0);
    assert_eq!(tally.raw_wpm(minute), 5.0);
    assert_eq!(tally.accuracy(), 80.0);
}

#[test]
fn stats_are_derived_from_the_tally() {
    let tally = Tally {
        typed: 12,
        correct: 10,
        indentation: 4,
        keystrokes: 9,
        errors: 1,
    };
    let elapsed = Duration::from_secs(6);
    let stats = Stats::new(tally, elapsed, 0.5);
    assert_eq!(stats.wpm, tally.wpm(elapsed));
    assert_eq!(stats.raw_wpm, tally.raw_wpm(elapsed));
    assert_eq!(stats.accuracy, tally.accuracy());
    assert_eq!(stats.errors, 1);
    assert_eq!(stats.correct_chars, 10);
    assert_eq!(stats.incorrect_chars, 2);
    assert_eq!(stats.keystrokes, 9);
    assert_eq!(stats.indentation, 4);
    assert_eq!(stats.progress, 0.5);
}

#[test]
fn completion_is_the_share_of_the_text_currently_correct() {
    assert_eq!(completion(3, 4), 0.75);
    assert_eq!(completion(4, 4), 1.0);
    assert_eq!(completion(0, 0), 0.0);
    assert_eq!(completion(5, 4), 1.0);
}

#[test]
fn an_empty_tally_is_neutral() {
    let tally = Tally::default();
    assert_eq!(tally.wpm(Duration::from_secs(10)), 0.0);
    assert_eq!(tally.accuracy(), 100.0);
}

#[test]
fn samples_cover_seconds_and_fold_short_remainders() {
    let millis = Duration::from_millis;
    assert!(sample_ends(Duration::ZERO).is_empty());
    assert_eq!(sample_ends(millis(300)), [millis(300)]);
    assert_eq!(sample_ends(millis(2_000)), [millis(1_000), millis(2_000)]);
    assert_eq!(sample_ends(millis(2_499)), [millis(1_000), millis(2_499)]);
    assert_eq!(
        sample_ends(millis(2_500)),
        [millis(1_000), millis(2_000), millis(2_500)]
    );
}

#[test]
fn consistency_rewards_steady_speed() {
    let steady: Vec<Sample> = (1..=5).map(|second| sample(second, 80.0)).collect();
    let erratic: Vec<Sample> = [20.0, 140.0, 30.0, 150.0, 10.0]
        .into_iter()
        .zip(1..)
        .map(|(speed, second)| sample(second, speed))
        .collect();
    assert_eq!(consistency(&steady), 100.0);
    assert!(consistency(&erratic) < 40.0);
}

#[test]
fn fewer_than_two_samples_are_consistent() {
    assert_eq!(consistency(&[]), 100.0);
    assert_eq!(consistency(&[sample(1, 70.0)]), 100.0);
}

fn sample(second: u32, raw_wpm: f64) -> Sample {
    Sample {
        second,
        wpm: raw_wpm,
        raw_wpm,
        errors: 0,
    }
}
