use clap::CommandFactory;

use super::*;

#[test]
fn no_subcommand_opens_home() {
    assert_eq!(launch(&[], &Practice::default()), Ok(Launch::Home));
}

#[test]
fn global_flags_are_accepted_after_a_subcommand() {
    let cli = Cli::try_parse_from([
        "code-racer",
        "history",
        "--theme",
        "mono",
        "--server",
        "lan:9000",
    ])
    .expect("valid arguments");
    assert_eq!(cli.theme, Some(Theme::Mono));
    assert_eq!(cli.server.as_deref(), Some("lan:9000"));
    assert_eq!(cli.launch(&Config::default()), Ok(Launch::History));
}

#[test]
fn join_accepts_lowercase_codes_and_rejects_invalid_ones() {
    assert_eq!(
        launch(&["join", "fk72ad"], &Practice::default()),
        Ok(Launch::Join("FK72AD".parse().expect("code")))
    );
    assert!(Cli::try_parse_from(["code-racer", "join", "FK72A0"]).is_err());
    assert!(Cli::try_parse_from(["code-racer", "join"]).is_err());
}

#[test]
fn multiplayer_opens_the_menu() {
    assert_eq!(
        launch(&["multiplayer"], &Practice::default()),
        Ok(Launch::Multiplayer)
    );
}

#[test]
fn create_help_only_offers_raceable_modes() {
    let mut command = Cli::command();
    let help = command
        .find_subcommand_mut("create")
        .expect("create subcommand")
        .render_help()
        .to_string();
    assert!(help.contains("words, quote or code"), "{help}");
    assert!(!help.contains("time"), "{help}");
}

#[test]
fn every_subcommand_and_flag_is_documented() {
    let mut command = Cli::command();
    command.build();
    for command in std::iter::once(&command).chain(command.get_subcommands()) {
        let name = command.get_name();
        assert!(command.get_about().is_some(), "{name} has no description");
        for argument in command.get_arguments() {
            let id = argument.get_id();
            assert!(
                argument.get_help().is_some(),
                "{name} {id} has no description"
            );
        }
    }
}
