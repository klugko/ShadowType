/*!
 * Crash-safe writes, locking and recovery of unusable files, shared by the
 * configuration and the history.
 */

mod recovery;
#[cfg(test)]
pub(crate) mod scratch;
#[cfg(test)]
mod tests;

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub use recovery::{Recovered, read_or_recover};

/**
 * A value read from disk, with an explanation for the user when the file was
 * unusable and had to be replaced.
 */
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loaded<T> {
    pub value: T,
    pub warning: Option<String>,
}

impl<T> Loaded<T> {
    pub fn clean(value: T) -> Self {
        Self {
            value,
            warning: None,
        }
    }

    pub fn map<U>(self, convert: impl FnOnce(T) -> U) -> Loaded<U> {
        Loaded {
            value: convert(self.value),
            warning: self.warning,
        }
    }
}

/**
 * Replaces the contents of `path`, creating its directory if needed.
 *
 * The bytes are written and flushed to a temporary sibling that is then
 * renamed over `path`, so a crash never leaves a truncated file behind.
 * A symbolic link is replaced through, not over, so that a file kept in a
 * dotfiles repository stays linked.
 */
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    create_parent_directory(&path)?;
    let temporary = with_suffix(&path, &format!(".{}.tmp", std::process::id()));
    let result = write_synced(&temporary, bytes).and_then(|()| fs::rename(&temporary, &path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/**
 * Waits for the advisory lock that guards `path` against other running
 * instances of code-racer, held until the returned file is dropped.
 *
 * The lock is taken on a `<name>.lock` sibling rather than on `path`, which
 * [`write_atomically`] replaces by another file.
 */
pub fn lock(path: &Path) -> io::Result<File> {
    let lock_path = with_suffix(path, ".lock");
    create_parent_directory(&lock_path)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    fs4::FileExt::lock(&file)?;
    Ok(file)
}

/// The contents of `path`, `None` when the file does not exist.
pub fn read_existing(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/**
 * The error for a file that does not hold what it should. Such a file is
 * refused rather than overwritten, so that the user can repair it.
 */
pub fn invalid_contents(problem: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("the file is invalid ({problem}), fix or delete it first"),
    )
}

fn create_parent_directory(path: &Path) -> io::Result<()> {
    match path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(parent) => fs::create_dir_all(parent),
        None => Ok(()),
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}
