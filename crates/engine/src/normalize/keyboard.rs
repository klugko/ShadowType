/// What a keyboard types for one character, in the form texts take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyboardForm {
    /// The character itself or its plain equivalent, such as `'` for `’`.
    Char(char),
    /// An ASCII spelling of a symbol, such as `->` for `→`.
    Spelled(&'static str),
    /// The character displays as nothing.
    Nothing,
}

impl KeyboardForm {
    pub(crate) fn chars(self) -> impl Iterator<Item = char> {
        let (single, spelled) = match self {
            Self::Char(ch) => (Some(ch), ""),
            Self::Spelled(spelling) => (None, spelling),
            Self::Nothing => (None, ""),
        };
        single.into_iter().chain(spelled.chars())
    }
}

/**
 * The one table that folds a character into what a keyboard types. Texts
 * go through it when they are normalised, and typed characters before
 * they are compared with the text, so that a typed `’` or no-break space
 * matches the `'` or space of the text. Tabs and line feeds stay.
 */
pub(crate) fn keyboard_form(ch: char) -> KeyboardForm {
    match ch {
        '\t' | '\n' => KeyboardForm::Char(ch),
        ch if ch.is_control() || is_invisible(ch) => KeyboardForm::Nothing,
        ch if ch.is_whitespace() => KeyboardForm::Char(' '),
        ch => plain_equivalent(ch)
            .map(KeyboardForm::Char)
            .or_else(|| ascii_spelling(ch).map(KeyboardForm::Spelled))
            .unwrap_or(KeyboardForm::Char(ch)),
    }
}

/**
 * The character a keyboard types in place of `ch`, when `ch` is a
 * typographic, compatibility or drawing variant of it.
 */
fn plain_equivalent(ch: char) -> Option<char> {
    let plain = match ch {
        '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' | '\u{2032}' | '\u{2039}'
        | '\u{203a}' | '\u{b4}' | '\u{2bc}' => '\'',
        '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' | '\u{2033}' | '\u{ab}' | '\u{bb}' => '"',
        '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
        '\u{2022}' | '\u{2023}' | '\u{2043}' | '\u{2219}' | '\u{25e6}' => '*',
        '\u{d7}' => 'x',
        '\u{f7}' | '\u{2044}' | '\u{2215}' => '/',
        '\u{2500}'..='\u{257f}' => box_drawing_stroke(ch),
        '\u{2580}'..='\u{259f}' => '#',
        '\u{ff01}'..='\u{ff5e}' => fullwidth_ascii(ch)?,
        _ => return None,
    };
    Some(plain)
}

fn box_drawing_stroke(ch: char) -> char {
    match ch {
        '─' | '━' | '┄' | '┅' | '┈' | '┉' | '═' | '╌' | '╍' | '╴' | '╶' | '╸' | '╺' | '╼' | '╾' => {
            '-'
        }
        '│' | '┃' | '┆' | '┇' | '┊' | '┋' | '║' | '╎' | '╏' | '╵' | '╷' | '╹' | '╻' | '╽' | '╿' => {
            '|'
        }
        '╱' => '/',
        '╲' => '\\',
        _ => '+',
    }
}

/**
 * The ASCII character that a fullwidth form, as CJK input methods type
 * it, stands for.
 */
fn fullwidth_ascii(ch: char) -> Option<char> {
    const OFFSET: u32 = '\u{ff01}' as u32 - '!' as u32;
    char::from_u32(u32::from(ch) - OFFSET)
}

/// How the symbols and ligatures that keyboards lack are spelled in ASCII.
fn ascii_spelling(ch: char) -> Option<&'static str> {
    let spelling = match ch {
        '\u{2026}' => "...",
        '→' => "->",
        '←' => "<-",
        '↔' => "<->",
        '⇒' => "=>",
        '⇐' => "<=",
        '⇔' => "<=>",
        '⟶' => "-->",
        '⟵' => "<--",
        '≤' => "<=",
        '≥' => ">=",
        '≠' => "!=",
        '≈' => "~=",
        '©' => "(c)",
        '®' => "(r)",
        '™' => "(tm)",
        'ﬀ' => "ff",
        'ﬁ' => "fi",
        'ﬂ' => "fl",
        'ﬃ' => "ffi",
        'ﬄ' => "ffl",
        'ﬅ' | 'ﬆ' => "st",
        _ => return None,
    };
    Some(spelling)
}

/// Formatting characters that display as nothing and that no key produces.
fn is_invisible(ch: char) -> bool {
    matches!(
        ch,
        '\u{ad}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{200b}'
            | '\u{200c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
    )
}

/**
 * Symbols that no keyboard layout or input method types and that have no
 * ASCII spelling: the replacement characters left by lossy decoding,
 * private-use icons such as those of patched fonts, and the technical and
 * pictographic blocks from Arrows to Miscellaneous Symbols and Arrows.
 * Circled and parenthesised numbers and letters stay, as input methods
 * type them, and so do emoji.
 */
pub(super) fn has_no_key(ch: char) -> bool {
    match ch {
        '\u{fffc}' | '\u{fffd}' | '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{10ffff}' => true,
        '\u{2460}'..='\u{24ff}' => false,
        '\u{2190}'..='\u{2bff}' => !is_emoji(ch),
        _ => false,
    }
}

/**
 * Symbols of the blocks [`has_no_key`] covers that are shown as emoji
 * without a presentation selector, as emoji pickers type them.
 */
fn is_emoji(ch: char) -> bool {
    matches!(
        ch,
        '\u{231a}'..='\u{231b}'
            | '\u{23e9}'..='\u{23ec}'
            | '\u{23f0}'
            | '\u{23f3}'
            | '\u{25fd}'..='\u{25fe}'
            | '\u{2614}'..='\u{2615}'
            | '\u{2648}'..='\u{2653}'
            | '\u{267f}'
            | '\u{2693}'
            | '\u{26a1}'
            | '\u{26aa}'..='\u{26ab}'
            | '\u{26bd}'..='\u{26be}'
            | '\u{26c4}'..='\u{26c5}'
            | '\u{26ce}'
            | '\u{26d4}'
            | '\u{26ea}'
            | '\u{26f2}'..='\u{26f3}'
            | '\u{26f5}'
            | '\u{26fa}'
            | '\u{26fd}'
            | '\u{2705}'
            | '\u{270a}'..='\u{270b}'
            | '\u{2728}'
            | '\u{274c}'
            | '\u{274e}'
            | '\u{2753}'..='\u{2755}'
            | '\u{2757}'
            | '\u{2795}'..='\u{2797}'
            | '\u{27b0}'
            | '\u{27bf}'
            | '\u{2b1b}'..='\u{2b1c}'
            | '\u{2b50}'
            | '\u{2b55}'
    )
}
