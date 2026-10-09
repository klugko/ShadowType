use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use code_racer_engine::{CodeLanguage, normalize};
use unicode_segmentation::UnicodeSegmentation;

use super::{CustomText, FileError};

const MAX_FILE_GRAPHEMES: usize = 3_000;
/**
 * Bytes read from a file at most: plenty for [`MAX_FILE_GRAPHEMES`], and
 * a huge file or an endless device cannot freeze the interface.
 */
const MAX_FILE_BYTES: usize = 64 * 1024;

impl CustomText {
    /// Loads a file, keeping at most the first few thousand characters, cut at a line break.
    pub fn load(path: &Path) -> Result<Self, FileError> {
        let display = path.display().to_string();
        let bytes = read_start(path, &display)?;
        let raw = decode(&bytes).ok_or_else(|| FileError::Binary(display.clone()))?;
        let normalized = normalize(raw);
        let text = truncate_at_line(&normalized, MAX_FILE_GRAPHEMES);
        if text.is_empty() {
            return Err(FileError::Empty(display));
        }
        Ok(Self {
            name: file_name(path),
            text: text.to_owned(),
            language: path
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(CodeLanguage::from_extension),
        })
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| PathBuf::from("untitled"), PathBuf::from)
        .display()
        .to_string()
}

/**
 * The first [`MAX_FILE_BYTES`] of a regular file, cut after a line break
 * when the file is longer.
 *
 * The type is checked before opening: opening a FIFO waits for a writer,
 * and reading a terminal or `/dev/zero` never ends.
 */
fn read_start(path: &Path, display: &str) -> Result<Vec<u8>, FileError> {
    let unreadable = |source| FileError::Unreadable {
        path: display.to_owned(),
        source,
    };
    if !fs::metadata(path).map_err(unreadable)?.is_file() {
        return Err(FileError::NotAFile(display.to_owned()));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(unreadable)?;
    if bytes.len() > MAX_FILE_BYTES {
        let cut = bytes[..MAX_FILE_BYTES]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .unwrap_or(MAX_FILE_BYTES);
        bytes.truncate(cut);
    }
    Ok(bytes)
}

/**
 * The text in `bytes`, `None` when they are not text. A character cut in
 * two by [`read_start`] at the end is dropped rather than making a valid
 * file look binary.
 */
fn decode(bytes: &[u8]) -> Option<&str> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() => {
            std::str::from_utf8(&bytes[..error.valid_up_to()]).ok()?
        }
        Err(_) => return None,
    };
    (!text.contains('\0')).then_some(text)
}

/**
 * The first `limit` characters of `text`, cut after the last line break
 * among them when the text is longer.
 */
fn truncate_at_line(text: &str, limit: usize) -> &str {
    let Some((end, _)) = text.grapheme_indices(true).nth(limit) else {
        return text;
    };
    let kept = &text[..end];
    kept.rfind('\n').map_or(kept, |cut| &kept[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Disguise, practice::Plan},
        persist::scratch::TempDir,
    };

    #[test]
    fn custom_files_are_normalised_and_typed_with_syntax() {
        let directory = TempDir::new();
        let path = directory.join("lib.rs");
        fs::write(&path, "fn main() {\r\n\tlet x = 1;   \r\n}\r\n").expect("write");
        let custom = CustomText::load(&path).expect("load");
        assert_eq!(custom.text, "fn main() {\n    let x = 1;\n}");
        assert_eq!(custom.language, Some(CodeLanguage::Rust));
        assert_eq!(Plan::File(custom).title(Disguise::default()), "lib.rs");
        fs::write(&path, [0u8, 159, 146, 150]).expect("write binary");
        assert!(matches!(CustomText::load(&path), Err(FileError::Binary(_))));
    }

    #[test]
    fn long_files_are_cut_at_a_line_break() {
        let text = "abc\n".repeat(1_000);
        assert_eq!(truncate_at_line(&text, 10), "abc\nabc");
        assert_eq!(truncate_at_line("short", 10), "short");
    }

    #[test]
    fn huge_files_are_read_only_up_to_the_limit() {
        let directory = TempDir::new();
        let path = directory.join("huge.log");
        fs::write(&path, "abc\n".repeat(MAX_FILE_BYTES)).expect("write");
        let custom = CustomText::load(&path).expect("load");
        assert!(custom.text.ends_with("abc"), "whole lines are kept");
        assert!(custom.text.graphemes(true).count() <= MAX_FILE_GRAPHEMES);
    }

    #[test]
    fn a_character_cut_by_the_byte_limit_does_not_make_the_file_binary() {
        let directory = TempDir::new();
        let path = directory.join("accents.txt");
        let text = format!("a{}", "é".repeat(MAX_FILE_BYTES));
        assert!(!text.is_char_boundary(MAX_FILE_BYTES));
        fs::write(&path, text).expect("write");
        let custom = CustomText::load(&path).expect("load");
        assert!(custom.text.starts_with("aé"));
    }

    #[test]
    fn only_regular_files_are_read() {
        let directory = TempDir::new();
        assert!(matches!(
            CustomText::load(directory.path()),
            Err(FileError::NotAFile(_))
        ));
        assert!(matches!(
            CustomText::load(&directory.join("missing.rs")),
            Err(FileError::Unreadable { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn devices_and_pipes_are_refused_without_blocking() {
        let directory = TempDir::new();
        let fifo = directory.join("pipe");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|status| status.success());
        let mut refused = vec![Path::new("/dev/zero").to_owned()];
        refused.extend(made.then_some(fifo));
        for path in refused {
            assert!(
                matches!(CustomText::load(&path), Err(FileError::NotAFile(_))),
                "{}",
                path.display()
            );
        }
    }
}
