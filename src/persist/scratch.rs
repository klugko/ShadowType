use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::recovery::{MAX_BACKUPS, backup_path};

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

/// Takes every backup name of `path`, so that it cannot be moved aside.
pub(crate) fn occupy_every_backup(path: &Path) {
    for number in 1..=MAX_BACKUPS {
        fs::write(backup_path(path, number), "older backup").expect("write backup");
    }
}

/**
 * Removes every permission on `path`. Returns `false` when the file can
 * still be read anyway, as it can by root.
 */
#[cfg(unix)]
pub(crate) fn make_unreadable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    fs::read(path).is_err()
}

#[cfg(unix)]
pub(crate) fn read_unreadable(path: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("chmod 600");
    fs::read_to_string(path).expect("read")
}
