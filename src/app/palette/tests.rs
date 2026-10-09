use super::*;

fn titles(found: &[Match]) -> Vec<&str> {
    found
        .iter()
        .map(|found| found.entry.title.as_str())
        .collect()
}

#[test]
fn a_few_letters_find_the_entry() {
    let entries = vec![
        entry("Words: 25 words", "", Action::Restart),
        entry("Theme: Mono", "", Action::Restart),
        entry("Open history.log", "", Action::Restart),
    ];
    assert_eq!(
        titles(&matches(entries.clone(), "hist")),
        ["Open history.log"]
    );
    assert_eq!(titles(&matches(entries.clone(), "tm")), ["Theme: Mono"]);
    assert_eq!(matches(entries.clone(), "").len(), 3);
    assert!(matches(entries, "zzz").is_empty());
}

#[test]
fn words_that_start_with_the_letters_come_first() {
    let entries = vec![
        entry("Animations: turn off", "", Action::Restart),
        entry("Mouse: turn off", "", Action::Restart),
    ];
    assert_eq!(titles(&matches(entries, "mo")), ["Mouse: turn off"]);
}

#[test]
fn matches_tell_which_characters_matched() {
    let found = matches(vec![entry("Quote", "", Action::Restart)], "qte");
    assert_eq!(found[0].positions, [0, 3, 4]);
}

#[test]
fn letters_never_match_in_the_middle_of_another_word() {
    let entries = vec![
        entry("Code: Rust snippet", "", Action::Restart),
        entry("Time: 15 seconds", "", Action::Restart),
        entry("Mascot: hide", "", Action::Restart),
    ];
    assert_eq!(
        titles(&matches(entries.clone(), "cod")),
        ["Code: Rust snippet"]
    );
    assert_eq!(titles(&matches(entries, "crs")), ["Code: Rust snippet"]);
}

#[test]
fn toggles_offer_the_other_state() {
    let on = toggle("Numbers", true, "numbers", Setting::Numbers);
    assert_eq!(on.title, "Numbers: turn off");
    assert_eq!(on.shortcut, ":set nonumbers");
    assert_eq!(
        on.action,
        Action::Run(Command::Set(Setting::Numbers(false)))
    );
}

#[test]
fn selection_wraps_around() {
    let mut palette = CommandPalette::new();
    palette.step(false, 4);
    assert_eq!(palette.selected(4), 3);
    palette.step(true, 4);
    assert_eq!(palette.selected(4), 0);
    palette.select(10, 4);
    assert_eq!(palette.selected(4), 3);
}
