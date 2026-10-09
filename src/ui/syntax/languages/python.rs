use super::{DOUBLE_QUOTE, SINGLE_QUOTE};
use crate::ui::syntax::{Strings, Syntax, Words};

pub(super) static PYTHON: Syntax = Syntax {
    line_comment: "#",
    block_comments: false,
    strings: Strings {
        quotes: &[DOUBLE_QUOTE, SINGLE_QUOTE],
        triple_quotes: true,
        escapes: true,
        prefixes: "bfruBFRU",
        raw_strings: false,
        char_literals: false,
    },
    words: Words {
        keywords: &[&[
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "self", "try", "while", "with", "yield",
        ]],
        soft_keywords: &["case", "match", "type"],
        types: &[
            "bool",
            "bytearray",
            "bytes",
            "complex",
            "dict",
            "float",
            "frozenset",
            "int",
            "list",
            "object",
            "set",
            "str",
            "tuple",
        ],
        ignore_case: false,
        capitalized_types: true,
        macros: false,
        identifier_symbols: "",
    },
};
