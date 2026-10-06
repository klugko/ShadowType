//! Crash-safe writes and recovery of unusable files, shared by the
//! configuration and the history.

use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

const MAX_BACKUPS: u32 = 99;

/// A value read from disk, with an explanation for the user when the file was
/// unusable and had to be replaced.
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

/// Replaces the contents of `path`, creating its directory if needed.
///
/// The bytes are written and flushed to a temporary sibling that is then
/// renamed over `path`, so a crash never leaves a truncated file behind.
/// A symbolic link is replaced through, not over, so that a file kept in a
/// dotfiles repository stays linked.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let temporary = with_suffix(&path, &format!(".{}.tmp", std::process::id()));
    let result = write_synced(&temporary, bytes).and_then(|()| fs::rename(&temporary, &path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Renames an unusable file to `<name>.bak`, or `<name>.<n>.bak` when older
/// backups exist, so that it can be inspected and repaired by hand.
pub fn move_aside(path: &Path) -> io::Result<PathBuf> {
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

/// Reads and parses `path`.
///
/// A missing file yields `None` silently. A file that cannot be read or
/// parsed is moved aside and yields `None` with a warning that ends with
/// `fallback`, the description of what replaces it.
pub fn read_or_recover<T>(
    path: &Path,
    fallback: &str,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> Loaded<Option<T>> {
    let problem = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Loaded::clean(None),
        Err(error) => error.to_string(),
        Ok(contents) => match parse(&contents) {
            Ok(value) => return Loaded::clean(Some(value)),
            Err(problem) => problem,
        },
    };
    Loaded {
        value: None,
        warning: Some(recovery_warning(path, &problem, fallback)),
    }
}

fn recovery_warning(path: &Path, problem: &str, fallback: &str) -> String {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    match move_aside(path) {
        Ok(backup) => format!(
            "{name} was invalid ({problem}); {fallback}, backup at {}",
            backup.display()
        ),
        Err(error) => {
            format!(
                "{name} was invalid ({problem}); {fallback}, but it could not be backed up: {error}"
            )
        }
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn backup_path(path: &Path, number: u32) -> PathBuf {
    if number == 1 {
        with_suffix(path, ".bak")
    } else {
        with_suffix(path, &format!(".{number}.bak"))
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

#[cfg(test)]
pub(crate) mod scratch {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// A fresh directory under the system temporary directory, removed on drop.
    #[derive(Debug)]
    pub(crate) struct TempDir(PathBuf);

    impl TempDir {
        pub(crate) fn new() -> Self {
            let name = format!(
                "code-racer-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(name);
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create temporary directory");
            Self(path)
        }

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }

        pub(crate) fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        pub(crate) fn file_names(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&self.0)
                .expect("list temporary directory")
                .map(|entry| {
                    entry
                        .expect("directory entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{scratch::TempDir, *};

    fn parse_number(contents: &str) -> Result<u32, String> {
        contents
            .trim()
            .parse()
            .map_err(|_| "not a number".to_owned())
    }

    #[test]
    fn atomic_write_creates_directories_and_replaces_content() {
        let dir = TempDir::new();
        let path = dir.path().join("nested/deeper/file.txt");
        write_atomically(&path, b"first").expect("first write");
        write_atomically(&path, b"second").expect("second write");
        assert_eq!(fs::read_to_string(&path).expect("read"), "second");
    }

    #[test]
    fn atomic_write_leaves_no_temporary_file() {
        let dir = TempDir::new();
        write_atomically(&dir.join("data.json"), b"[]").expect("write");
        assert_eq!(dir.file_names(), ["data.json"]);
    }

    #[test]
    fn failed_atomic_write_keeps_the_target_and_cleans_up() {
        let dir = TempDir::new();
        let target = dir.join("occupied");
        fs::create_dir(&target).expect("create directory in the way");
        fs::write(target.join("keep.txt"), "precious").expect("write");

        assert!(write_atomically(&target, b"data").is_err());
        assert_eq!(dir.file_names(), ["occupied"]);
        assert_eq!(
            fs::read_to_string(target.join("keep.txt")).expect("read"),
            "precious"
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_through_a_symlink_keeps_the_link() {
        let dir = TempDir::new();
        let real = dir.join("dotfiles-config.toml");
        let link = dir.join("config.toml");
        fs::write(&real, "old").expect("write");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");

        write_atomically(&link, b"new").expect("write through the link");

        assert!(fs::symlink_metadata(&link).expect("link").is_symlink());
        assert_eq!(fs::read_to_string(&real).expect("read"), "new");
        assert_eq!(dir.file_names(), ["config.toml", "dotfiles-config.toml"]);
    }

    #[test]
    fn moving_aside_never_overwrites_an_older_backup() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        fs::write(&path, "broken once").expect("write");
        let first = move_aside(&path).expect("first backup");
        fs::write(&path, "broken twice").expect("write");
        let second = move_aside(&path).expect("second backup");

        assert_eq!(first, dir.join("config.toml.bak"));
        assert_eq!(second, dir.join("config.toml.2.bak"));
        assert_eq!(fs::read_to_string(first).expect("read"), "broken once");
        assert_eq!(fs::read_to_string(second).expect("read"), "broken twice");
        assert!(!path.exists());
    }

    #[test]
    fn missing_file_reads_as_none_without_warning() {
        let dir = TempDir::new();
        let loaded = read_or_recover(&dir.join("absent"), "defaults loaded", parse_number);
        assert_eq!(loaded, Loaded::clean(None));
        assert!(dir.file_names().is_empty());
    }

    #[test]
    fn valid_file_is_parsed_in_place() {
        let dir = TempDir::new();
        let path = dir.join("number");
        fs::write(&path, "42\n").expect("write");
        assert_eq!(
            read_or_recover(&path, "defaults loaded", parse_number),
            Loaded::clean(Some(42))
        );
        assert!(path.exists());
    }

    #[test]
    fn invalid_file_is_moved_aside_with_an_explanation() {
        let dir = TempDir::new();
        let path = dir.join("number");
        fs::write(&path, "forty-two").expect("write");

        let loaded = read_or_recover(&path, "defaults loaded", parse_number);

        let backup = dir.join("number.bak");
        assert_eq!(loaded.value, None);
        assert_eq!(
            loaded.warning,
            Some(format!(
                "number was invalid (not a number); defaults loaded, backup at {}",
                backup.display()
            ))
        );
        assert_eq!(fs::read_to_string(backup).expect("read"), "forty-two");
        assert!(!path.exists());
    }

    #[test]
    fn unreadable_file_is_moved_aside_too() {
        let dir = TempDir::new();
        let path = dir.join("binary");
        fs::write(&path, [0xff, 0xfe, 0x00]).expect("write");

        let loaded = read_or_recover(&path, "starting over", parse_number);

        assert_eq!(loaded.value, None);
        let warning = loaded.warning.expect("warning");
        assert!(warning.starts_with("binary was invalid ("), "{warning}");
        assert!(warning.contains("starting over, backup at"), "{warning}");
        assert!(dir.join("binary.bak").exists());
    }
}
