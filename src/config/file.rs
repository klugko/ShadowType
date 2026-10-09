use std::{io, path::Path};

use toml_edit::{DocumentMut, Item, TableLike, Value};

use super::{Config, Loaded};
use crate::persist::{self, Recovered};

/**
 * Reads the configuration. A missing file gives the defaults, and an
 * invalid one is moved aside, explained in the warning, and gives the
 * defaults too. `None` means the file could be neither used nor moved
 * aside: it is still there, and must not be written over.
 */
pub fn load_config(path: &Path) -> Loaded<Option<Config>> {
    persist::read_or_recover(path, "defaults loaded", parse_config).map(|found| match found {
        Recovered::Parsed(config) => Some(config),
        Recovered::Absent => Some(Config::default()),
        Recovered::LeftInPlace => None,
    })
}

/**
 * Applies `change` to the settings saved at `path`, as the file holds them
 * now, and writes them back when they changed. The values are updated in
 * place, so that the comments, layout and unknown keys of the file survive.
 *
 * A file that cannot be read, or that does not hold valid settings, is
 * refused rather than overwritten: it is what failed to load, and the
 * settings in memory are only defaults. Other instances of code-racer wait
 * while the file is read and replaced, so that none undoes another's save.
 */
pub fn update_config(path: &Path, change: impl FnOnce(&mut Config)) -> io::Result<()> {
    let _lock = persist::lock(path)?;
    let existing = persist::read_existing(path)?.unwrap_or_default();
    let saved = parse_config(&existing).map_err(|problem| persist::invalid_contents(&problem))?;
    let mut updated = saved.clone();
    change(&mut updated);
    if updated == saved {
        return Ok(());
    }
    let contents = updated_document(&existing, &updated)?;
    persist::write_atomically(path, contents.as_bytes())
}

fn updated_document(existing: &str, config: &Config) -> io::Result<String> {
    let mut document: DocumentMut = existing.parse().map_err(io::Error::other)?;
    let settings: DocumentMut = toml::to_string(config)
        .map_err(io::Error::other)?
        .parse()
        .map_err(io::Error::other)?;
    update_table(document.as_table_mut(), settings.as_table());
    Ok(document.to_string())
}

/**
 * Copies every setting into `table`, inserting the missing ones and
 * leaving keys that are not settings alone.
 */
fn update_table(table: &mut dyn TableLike, settings: &dyn TableLike) {
    for (key, setting) in settings.iter() {
        match table.get_mut(key) {
            Some(item) => update_item(item, setting),
            None => {
                table.insert(key, setting.clone());
            }
        }
    }
}

fn update_item(item: &mut Item, setting: &Item) {
    if let (Some(table), Some(settings)) = (item.as_table_like_mut(), setting.as_table_like()) {
        update_table(table, settings);
    } else if let (Some(value), Some(new_value)) = (item.as_value_mut(), setting.as_value()) {
        update_value(value, new_value);
    } else {
        *item = setting.clone();
    }
}

/**
 * Replaces a changed value, keeping the whitespace and comment around it.
 * An unchanged one keeps its spelling too, such as single quotes.
 */
fn update_value(value: &mut Value, new_value: &Value) {
    if !same_setting(value, new_value) {
        let decor = value.decor().clone();
        *value = new_value.clone();
        *value.decor_mut() = decor;
    }
}

/// Settings are only ever strings, integers or booleans.
fn same_setting(value: &Value, other: &Value) -> bool {
    match (value, other) {
        (Value::String(value), Value::String(other)) => value.value() == other.value(),
        (Value::Integer(value), Value::Integer(other)) => value.value() == other.value(),
        (Value::Boolean(value), Value::Boolean(other)) => value.value() == other.value(),
        _ => false,
    }
}

fn parse_config(contents: &str) -> Result<Config, String> {
    let config: Config = toml::from_str(contents).map_err(|error| describe(&error, contents))?;
    Ok(Config {
        practice: config.practice.sanitized(),
        race: config.race.sanitized().for_race(),
        ..config
    })
}

fn describe(error: &toml::de::Error, contents: &str) -> String {
    let message = error
        .message()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    match error.span() {
        Some(span) => {
            let line = contents
                .bytes()
                .take(span.start)
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            format!("line {line}: {message}")
        }
        None => message,
    }
}
