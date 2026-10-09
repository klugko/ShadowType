use super::{
    recovery::move_aside,
    scratch::{TempDir, occupy_every_backup},
    *,
};

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
fn lock_is_held_until_dropped() {
    let dir = TempDir::new();
    let path = dir.join("data/history.json");
    let other_handle = || {
        File::options()
            .write(true)
            .open(dir.join("data/history.json.lock"))
            .expect("open the lock file")
    };

    let held = lock(&path).expect("lock");
    assert!(fs4::FileExt::try_lock(&other_handle()).is_err());

    drop(held);
    assert!(fs4::FileExt::try_lock(&other_handle()).is_ok());
    assert!(!path.exists());
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
    assert_eq!(loaded, Loaded::clean(Recovered::Absent));
    assert!(dir.file_names().is_empty());
}

#[test]
fn valid_file_is_parsed_in_place() {
    let dir = TempDir::new();
    let path = dir.join("number");
    fs::write(&path, "42\n").expect("write");
    assert_eq!(
        read_or_recover(&path, "defaults loaded", parse_number),
        Loaded::clean(Recovered::Parsed(42))
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
    assert_eq!(loaded.value, Recovered::Absent);
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
fn file_that_is_not_text_is_moved_aside_too() {
    let dir = TempDir::new();
    let path = dir.join("binary");
    fs::write(&path, [0xff, 0xfe, 0x00]).expect("write");

    let loaded = read_or_recover(&path, "starting over", parse_number);

    assert_eq!(loaded.value, Recovered::Absent);
    let warning = loaded.warning.expect("warning");
    assert!(warning.starts_with("binary was invalid ("), "{warning}");
    assert!(warning.contains("starting over, backup at"), "{warning}");
    assert!(dir.join("binary.bak").exists());
}

#[test]
fn invalid_file_without_a_free_backup_name_is_left_in_place() {
    let dir = TempDir::new();
    let path = dir.join("number");
    fs::write(&path, "forty-two").expect("write");
    occupy_every_backup(&path);

    let loaded = read_or_recover(&path, "defaults loaded", parse_number);

    assert_eq!(loaded.value, Recovered::LeftInPlace);
    assert_eq!(
        loaded.warning.as_deref(),
        Some(
            "number was invalid (not a number) and could not be backed up \
             (99 backups already exist); defaults loaded, and number will be left as it is"
        )
    );
    assert_eq!(fs::read_to_string(&path).expect("read"), "forty-two");
}

#[cfg(unix)]
#[test]
fn file_that_cannot_be_read_is_left_in_place() {
    let dir = TempDir::new();
    let path = dir.join("number");
    fs::write(&path, "42").expect("write");
    if !scratch::make_unreadable(&path) {
        return;
    }

    let loaded = read_or_recover(&path, "defaults loaded", parse_number);

    assert_eq!(loaded.value, Recovered::LeftInPlace);
    let warning = loaded.warning.expect("warning");
    assert!(warning.starts_with("cannot read number ("), "{warning}");
    assert!(
        warning.ends_with("); defaults loaded, and number will be left as it is"),
        "{warning}"
    );
    assert_eq!(dir.file_names(), ["number"]);
    assert_eq!(scratch::read_unreadable(&path), "42");
}
