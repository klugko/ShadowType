use std::{
    borrow::Cow,
    fs, io,
    path::{Path, PathBuf},
};

use super::{Loaded, with_suffix};

pub(super) const MAX_BACKUPS: u32 = 99;

/**
 * Renames an unusable file to `<name>.bak`, or `<name>.<n>.bak` when older
 * backups exist, so that it can be inspected and repaired by hand.
 */
pub(super) fn move_aside(path: &Path) -> io::Result<PathBuf> {
    let backup = (1..=MAX_BACKUPS)
        .map(|number| backup_path(path, number))
        .find(|candidate| !candidate.exists())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{MAX_BACKUPS} backups already exist"),
            )
        })?;
    fs::rename(path, &backup)?;
    Ok(backup)
}

/// What [`read_or_recover`] found at a path.
#[derive(Debug, Clone, PartialEq)]
pub enum Recovered<T> {
    Parsed(T),
    /**
     * No usable file: it did not exist, or it was invalid and has been moved
     * aside. A new one may be written.
     */
    Absent,
    /**
     * The file could not be used but is still there, because it could not
     * be read or moved aside. Writing to the path would destroy it.
     */
    LeftInPlace,
}

/**
 * Reads and parses `path`.
 *
 * A missing file is [`Recovered::Absent`] silently. Any other problem comes
 * with a warning that continues with `fallback`, the description of what
 * replaces the file. Only a file whose contents are invalid is moved aside:
 * one that cannot be read at all, for lack of permission or because another
 * program holds it, may be perfectly fine and is left alone.
 */
pub fn read_or_recover<T>(
    path: &Path,
    fallback: &str,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> Loaded<Recovered<T>> {
    let problem = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Loaded::clean(Recovered::Absent);
        }
        Err(error) if error.kind() != io::ErrorKind::InvalidData => {
            let problem = format!("cannot read {} ({error})", file_name(path));
            return left_in_place(path, &problem, fallback);
        }
        Err(error) => error.to_string(),
        Ok(contents) => match parse(&contents) {
            Ok(value) => return Loaded::clean(Recovered::Parsed(value)),
            Err(problem) => problem,
        },
    };
    recover(path, &problem, fallback)
}

fn recover<T>(path: &Path, problem: &str, fallback: &str) -> Loaded<Recovered<T>> {
    let name = file_name(path);
    match move_aside(path) {
        Ok(backup) => Loaded {
            value: Recovered::Absent,
            warning: Some(format!(
                "{name} was invalid ({problem}); {fallback}, backup at {}",
                backup.display()
            )),
        },
        Err(error) => {
            let problem =
                format!("{name} was invalid ({problem}) and could not be backed up ({error})");
            left_in_place(path, &problem, fallback)
        }
    }
}

fn left_in_place<T>(path: &Path, problem: &str, fallback: &str) -> Loaded<Recovered<T>> {
    Loaded {
        value: Recovered::LeftInPlace,
        warning: Some(format!(
            "{problem}; {fallback}, and {} will be left as it is",
            file_name(path)
        )),
    }
}

fn file_name(path: &Path) -> Cow<'_, str> {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
}

pub(super) fn backup_path(path: &Path, number: u32) -> PathBuf {
    if number == 1 {
        with_suffix(path, ".bak")
    } else {
        with_suffix(path, &format!(".{number}.bak"))
    }
}
