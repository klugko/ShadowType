use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Judgement {
    Correct,
    /// The beginning of the expected character, such as `e` on the way to `é`.
    Partial,
    Wrong,
}

/// Splits `text` into NFC-normalised extended grapheme clusters.
pub fn graphemes(text: &str) -> Vec<String> {
    let composed: String = text.nfc().collect();
    composed.graphemes(true).map(str::to_owned).collect()
}

/// Number of characters a player has to type for `text`.
pub fn grapheme_count(text: &str) -> usize {
    let composed: String = text.nfc().collect();
    composed.graphemes(true).count()
}

pub(super) fn judge(typed: &str, expected: &str) -> Judgement {
    if same_text(typed, expected) {
        Judgement::Correct
    } else if is_partial(typed, expected) {
        Judgement::Partial
    } else {
        Judgement::Wrong
    }
}

fn same_text(typed: &str, expected: &str) -> bool {
    typed.nfc().eq(expected.nfc())
}

/// Whether `typed` is the beginning of `expected`, such as `e` for `é`.
fn is_partial(typed: &str, expected: &str) -> bool {
    let typed: String = typed.nfd().collect();
    let expected: String = expected.nfd().collect();
    expected.len() > typed.len() && expected.starts_with(&typed)
}
