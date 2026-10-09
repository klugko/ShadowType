mod commands;
mod races;
mod solo;

use super::*;
use crate::config::Config;

/**
 * Launches with `defaults` saved for solo sessions and, made raceable,
 * for races.
 */
fn launch(arguments: &[&str], defaults: &Practice) -> Result<Launch, CliError> {
    let saved = Config {
        practice: *defaults,
        race: defaults.for_race(),
        ..Config::default()
    };
    let cli = Cli::try_parse_from(["code-racer"].iter().chain(arguments)).expect("valid arguments");
    cli.launch(&saved)
}

fn solo(arguments: &[&str], defaults: &Practice) -> Practice {
    let mut command = vec!["solo"];
    command.extend_from_slice(arguments);
    match launch(&command, defaults) {
        Ok(Launch::Solo { practice, .. }) => practice,
        other => panic!("expected a solo launch, got {other:?}"),
    }
}

fn create(arguments: &[&str], defaults: &Practice) -> Result<TextSource, CliError> {
    let mut command = vec!["create"];
    command.extend_from_slice(arguments);
    launch(&command, defaults).map(|launch| match launch {
        Launch::Create(text) => text,
        other => panic!("expected a create launch, got {other:?}"),
    })
}

fn defaults_with_mode(mode: Mode) -> Practice {
    Practice {
        mode,
        ..Practice::default()
    }
}
