use crate::ui::syntax::{Quote, Strings, Syntax, Words};

pub(super) static RUST: Syntax = Syntax {
    line_comment: "//",
    block_comments: true,
    strings: Strings {
        quotes: &[Quote {
            mark: "\"",
            multiline: true,
        }],
        triple_quotes: false,
        escapes: true,
        prefixes: "br",
        raw_strings: true,
        char_literals: true,
    },
    words: Words {
        keywords: &[&[
            "Self", "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
            "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
            "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while",
        ]],
        soft_keywords: &[],
        types: &[
            "bool", "char", "f32", "f64", "i128", "i16", "i32", "i64", "i8", "isize", "str",
            "u128", "u16", "u32", "u64", "u8", "usize",
        ],
        ignore_case: false,
        capitalized_types: true,
        macros: true,
        identifier_symbols: "",
    },
};
