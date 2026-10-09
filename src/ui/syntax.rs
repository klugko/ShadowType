//! Syntax highlighting of code snippets: one hand-written lexer driven by a
//! small table per language, aimed at idiomatic snippets rather than full
//! grammars. It is total: any input, half-typed or malformed, gets exactly one
//! token per grapheme.

use std::iter;

use code_racer_engine::CodeLanguage;

/// Syntax class of one grapheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Plain,
    Keyword,
    Type,
    Function,
    Macro,
    String,
    Number,
    Comment,
    Punctuation,
}

/// Classifies every grapheme of `graphemes`; the result has the same length.
pub fn highlight(graphemes: &[String], language: CodeLanguage) -> Vec<Token> {
    Lexer {
        graphemes,
        syntax: Syntax::of(language),
        tokens: Vec::with_capacity(graphemes.len()),
    }
    .run()
}

#[derive(Debug)]
struct Syntax {
    line_comment: &'static str,
    block_comments: bool,
    strings: Strings,
    words: Words,
}

#[derive(Debug)]
struct Strings {
    quotes: &'static [Quote],
    triple_quotes: bool,
    escapes: bool,
    /// Letters that may prefix a string literal, such as `b` in `b"bytes"`.
    prefixes: &'static str,
    /// Whether an `r` prefix makes a Rust raw string: no escapes, optional `#` fences.
    raw_strings: bool,
    /// Whether `'` starts a Rust character literal or lifetime instead of a string.
    char_literals: bool,
}

#[derive(Debug, Clone, Copy)]
struct Quote {
    mark: &'static str,
    multiline: bool,
}

#[derive(Debug)]
struct Words {
    keywords: &'static [&'static [&'static str]],
    /// Keywords only where they open a statement, like Python's `match`.
    soft_keywords: &'static [&'static str],
    types: &'static [&'static str],
    /// Keyword and type lists are lowercase and matched case-insensitively.
    ignore_case: bool,
    capitalized_types: bool,
    macros: bool,
    identifier_symbols: &'static str,
}

impl Syntax {
    fn of(language: CodeLanguage) -> &'static Self {
        match language {
            CodeLanguage::Rust => &RUST,
            CodeLanguage::Python => &PYTHON,
            CodeLanguage::TypeScript => &TYPESCRIPT,
            CodeLanguage::JavaScript => &JAVASCRIPT,
            CodeLanguage::Sql => &SQL,
        }
    }
}

impl Words {
    fn is_keyword(&self, word: &[String]) -> bool {
        self.keywords.iter().any(|list| self.contains(list, word))
    }

    fn is_type(&self, word: &[String]) -> bool {
        self.contains(self.types, word)
            || (self.capitalized_types
                && word
                    .first()
                    .and_then(|g| first_char(g))
                    .is_some_and(char::is_uppercase))
    }

    fn contains(&self, sorted: &[&str], word: &[String]) -> bool {
        let bytes = || word.iter().flat_map(|grapheme| grapheme.bytes());
        sorted
            .binary_search_by(|candidate| {
                if self.ignore_case {
                    candidate
                        .bytes()
                        .cmp(bytes().map(|byte| byte.to_ascii_lowercase()))
                } else {
                    candidate.bytes().cmp(bytes())
                }
            })
            .is_ok()
    }
}

#[derive(Debug, Clone, Copy)]
struct Literal {
    quote: &'static str,
    repeat: usize,
    hashes: usize,
    escapes: bool,
    multiline: bool,
}

const CHAR_LITERAL: Literal = Literal {
    quote: "'",
    repeat: 1,
    hashes: 0,
    escapes: true,
    multiline: false,
};

struct Lexer<'a> {
    graphemes: &'a [String],
    syntax: &'static Syntax,
    tokens: Vec<Token>,
}

impl Lexer<'_> {
    fn run(mut self) -> Vec<Token> {
        while self.tokens.len() < self.graphemes.len() {
            let (token, len) = self.lexeme();
            let len = len.clamp(1, self.rest().len());
            self.tokens.extend(iter::repeat_n(token, len));
        }
        self.tokens
    }

    fn rest(&self) -> &[String] {
        &self.graphemes[self.tokens.len()..]
    }

    fn at(&self, offset: usize) -> &str {
        self.rest().get(offset).map_or("", String::as_str)
    }

    fn lexeme(&self) -> (Token, usize) {
        if self.starts_with(self.syntax.line_comment) {
            return (Token::Comment, self.line_comment_len());
        }
        if self.syntax.block_comments && self.starts_with("/*") {
            return (Token::Comment, self.block_comment_len());
        }
        if let Some(len) = self.string_len() {
            return (Token::String, len);
        }
        if self.syntax.strings.char_literals && self.at(0) == "'" {
            return self.lifetime();
        }
        if starts_with_digit(self.at(0)) {
            return (Token::Number, self.number_len());
        }
        if self.is_identifier_start(self.at(0)) {
            return self.identifier();
        }
        let punctuation = first_char(self.at(0)).is_some_and(|c| c.is_ascii_punctuation());
        let token = if punctuation {
            Token::Punctuation
        } else {
            Token::Plain
        };
        (token, 1)
    }

    fn starts_with(&self, pattern: &str) -> bool {
        !pattern.is_empty()
            && pattern
                .chars()
                .enumerate()
                .all(|(offset, expected)| is_char(self.at(offset), expected))
    }

    fn line_comment_len(&self) -> usize {
        let rest = self.rest();
        rest.iter()
            .position(|grapheme| grapheme == "\n")
            .unwrap_or(rest.len())
    }

    fn block_comment_len(&self) -> usize {
        (2..self.rest().len())
            .find(|&offset| self.at(offset) == "*" && self.at(offset + 1) == "/")
            .map_or(self.rest().len(), |offset| offset + 2)
    }

    fn string_len(&self) -> Option<usize> {
        let strings = &self.syntax.strings;
        let prefix = self
            .rest()
            .iter()
            .take(2)
            .take_while(|grapheme| is_one_of(grapheme, strings.prefixes))
            .count();
        let raw = strings.raw_strings && self.rest()[..prefix].iter().any(|g| is_one_of(g, "rR"));
        let hashes = if raw { self.count_from(prefix, "#") } else { 0 };
        let open = prefix + hashes;
        if strings.char_literals && hashes == 0 && self.at(open) == "'" {
            return self.char_literal_len(open);
        }
        let quote = strings
            .quotes
            .iter()
            .find(|quote| self.at(open) == quote.mark)?;
        let triple = strings.triple_quotes
            && self.at(open + 1) == quote.mark
            && self.at(open + 2) == quote.mark;
        let literal = Literal {
            quote: quote.mark,
            repeat: if triple { 3 } else { 1 },
            hashes,
            escapes: strings.escapes && !raw,
            multiline: quote.multiline || triple,
        };
        Some(self.literal_end(open + literal.repeat, literal))
    }

    fn char_literal_len(&self, open: usize) -> Option<usize> {
        if self.at(open + 1) == "\\" {
            return Some(self.literal_end(open + 1, CHAR_LITERAL));
        }
        let single = !matches!(self.at(open + 1), "'" | "\n" | "");
        (single && self.at(open + 2) == "'").then_some(open + 3)
    }

    fn literal_end(&self, start: usize, literal: Literal) -> usize {
        let mut offset = start;
        while offset < self.rest().len() {
            let grapheme = self.at(offset);
            if literal.escapes && grapheme == "\\" {
                offset += 2;
            } else if grapheme == "\n" && !literal.multiline {
                return offset;
            } else if self.closes(offset, literal) {
                return offset + literal.repeat + literal.hashes;
            } else {
                offset += 1;
            }
        }
        self.rest().len()
    }

    fn closes(&self, offset: usize, literal: Literal) -> bool {
        (0..literal.repeat).all(|index| self.at(offset + index) == literal.quote)
            && (0..literal.hashes).all(|index| self.at(offset + literal.repeat + index) == "#")
    }

    fn count_from(&self, offset: usize, expected: &str) -> usize {
        self.rest()
            .iter()
            .skip(offset)
            .take_while(|grapheme| *grapheme == expected)
            .count()
    }

    fn lifetime(&self) -> (Token, usize) {
        match self.identifier_len(1) {
            0 => (Token::Punctuation, 1),
            name => (Token::Keyword, 1 + name),
        }
    }

    fn number_len(&self) -> usize {
        let hex = self.at(0) == "0" && is_one_of(self.at(1), "xX");
        let mut len = 1;
        let mut fraction = false;
        loop {
            let grapheme = self.at(len);
            let continues = match grapheme {
                "." => !fraction && !hex && starts_with_digit(self.at(len + 1)),
                "+" | "-" => !hex && self.is_exponent_marker(len - 1),
                _ => first_char(grapheme).is_some_and(|c| c.is_ascii_alphanumeric() || c == '_'),
            };
            if !continues {
                return len;
            }
            fraction |= grapheme == ".";
            len += 1;
        }
    }

    /// Whether the `e` at `offset` starts an exponent, as in `2.5e-3`, rather
    /// than ending a suffix, as in `1usize-1`.
    fn is_exponent_marker(&self, offset: usize) -> bool {
        is_one_of(self.at(offset), "eE")
            && offset
                .checked_sub(1)
                .is_some_and(|before| starts_with_digit(self.at(before)))
    }

    fn identifier(&self) -> (Token, usize) {
        let words = &self.syntax.words;
        let len = self.identifier_len(0);
        let word = &self.rest()[..len];
        if words.is_keyword(word) || self.is_soft_keyword(word) {
            return (Token::Keyword, len);
        }
        if words.macros && self.at(len) == "!" && self.at(len + 1) != "=" {
            return (Token::Macro, len + 1);
        }
        let token = if words.is_type(word) {
            Token::Type
        } else if self.at(len) == "(" {
            Token::Function
        } else {
            Token::Plain
        };
        (token, len)
    }

    /// Whether `word` is a soft keyword such as Python's `match`: one that opens
    /// a statement, is followed by a space and is not being assigned to, as in
    /// `match = pattern.search(line)`.
    fn is_soft_keyword(&self, word: &[String]) -> bool {
        let words = &self.syntax.words;
        let spaces = self.count_from(word.len(), " ");
        let next = word.len() + spaces;
        words.contains(words.soft_keywords, word)
            && spaces > 0
            && !matches!(self.at(next), "" | "\n")
            && !self.is_assignment_at(next)
            && self.starts_line()
    }

    fn is_assignment_at(&self, offset: usize) -> bool {
        let operator = self
            .rest()
            .iter()
            .skip(offset)
            .take_while(|grapheme| is_one_of(grapheme, "+-*/%&|^@<>"))
            .count();
        self.at(offset + operator) == "="
    }

    fn starts_line(&self) -> bool {
        self.graphemes[..self.tokens.len()]
            .iter()
            .rev()
            .take_while(|grapheme| *grapheme != "\n")
            .all(|grapheme| grapheme == " ")
    }

    fn identifier_len(&self, offset: usize) -> usize {
        if !self.is_identifier_start(self.at(offset)) {
            return 0;
        }
        1 + self
            .rest()
            .iter()
            .skip(offset + 1)
            .take_while(|grapheme| self.is_identifier_continue(grapheme))
            .count()
    }

    fn is_identifier_start(&self, grapheme: &str) -> bool {
        first_char(grapheme).is_some_and(|c| c.is_alphabetic() || c == '_')
            || is_one_of(grapheme, self.syntax.words.identifier_symbols)
    }

    fn is_identifier_continue(&self, grapheme: &str) -> bool {
        first_char(grapheme).is_some_and(|c| c.is_alphanumeric() || c == '_')
            || is_one_of(grapheme, self.syntax.words.identifier_symbols)
    }
}

fn first_char(grapheme: &str) -> Option<char> {
    grapheme.chars().next()
}

fn starts_with_digit(grapheme: &str) -> bool {
    first_char(grapheme).is_some_and(|c| c.is_ascii_digit())
}

fn is_char(grapheme: &str, expected: char) -> bool {
    let mut chars = grapheme.chars();
    chars.next() == Some(expected) && chars.next().is_none()
}

fn is_one_of(grapheme: &str, set: &str) -> bool {
    let mut chars = grapheme.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if set.contains(c))
}

const DOUBLE_QUOTE: Quote = Quote {
    mark: "\"",
    multiline: false,
};
const SINGLE_QUOTE: Quote = Quote {
    mark: "'",
    multiline: false,
};

static RUST: Syntax = Syntax {
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

static PYTHON: Syntax = Syntax {
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

const JAVASCRIPT_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

const JAVASCRIPT_STRINGS: Strings = Strings {
    quotes: &[
        DOUBLE_QUOTE,
        SINGLE_QUOTE,
        Quote {
            mark: "`",
            multiline: true,
        },
    ],
    triple_quotes: false,
    escapes: true,
    prefixes: "",
    raw_strings: false,
    char_literals: false,
};

static JAVASCRIPT: Syntax = Syntax {
    line_comment: "//",
    block_comments: true,
    strings: JAVASCRIPT_STRINGS,
    words: Words {
        keywords: &[JAVASCRIPT_KEYWORDS],
        soft_keywords: &[],
        types: &[],
        ignore_case: false,
        capitalized_types: true,
        macros: false,
        identifier_symbols: "$",
    },
};

static TYPESCRIPT: Syntax = Syntax {
    line_comment: "//",
    block_comments: true,
    strings: JAVASCRIPT_STRINGS,
    words: Words {
        keywords: &[
            JAVASCRIPT_KEYWORDS,
            &[
                "abstract",
                "asserts",
                "declare",
                "enum",
                "implements",
                "infer",
                "interface",
                "is",
                "keyof",
                "namespace",
                "override",
                "private",
                "protected",
                "public",
                "readonly",
                "satisfies",
                "type",
            ],
        ],
        soft_keywords: &[],
        types: &[
            "any", "bigint", "boolean", "never", "number", "object", "string", "symbol", "unknown",
        ],
        ignore_case: false,
        capitalized_types: true,
        macros: false,
        identifier_symbols: "$",
    },
};

static SQL: Syntax = Syntax {
    line_comment: "--",
    block_comments: true,
    strings: Strings {
        quotes: &[
            Quote {
                mark: "'",
                multiline: true,
            },
            Quote {
                mark: "\"",
                multiline: true,
            },
        ],
        triple_quotes: false,
        escapes: false,
        prefixes: "",
        raw_strings: false,
        char_literals: false,
    },
    words: Words {
        keywords: &[&[
            "add",
            "all",
            "alter",
            "always",
            "and",
            "any",
            "as",
            "asc",
            "before",
            "begin",
            "between",
            "by",
            "cascade",
            "case",
            "check",
            "column",
            "commit",
            "concurrently",
            "conflict",
            "constraint",
            "create",
            "cross",
            "current",
            "current_date",
            "current_timestamp",
            "data",
            "default",
            "delete",
            "desc",
            "distinct",
            "do",
            "drop",
            "each",
            "else",
            "end",
            "except",
            "excluded",
            "execute",
            "exists",
            "false",
            "filter",
            "following",
            "for",
            "foreign",
            "from",
            "full",
            "function",
            "generated",
            "group",
            "having",
            "identity",
            "if",
            "ilike",
            "in",
            "include",
            "index",
            "inner",
            "insert",
            "intersect",
            "into",
            "is",
            "join",
            "key",
            "language",
            "lateral",
            "left",
            "like",
            "limit",
            "locked",
            "materialized",
            "new",
            "not",
            "nothing",
            "null",
            "offset",
            "old",
            "on",
            "or",
            "order",
            "outer",
            "over",
            "partition",
            "preceding",
            "primary",
            "range",
            "recursive",
            "references",
            "replace",
            "return",
            "returning",
            "returns",
            "right",
            "rollback",
            "row",
            "rows",
            "select",
            "set",
            "skip",
            "table",
            "then",
            "to",
            "trigger",
            "true",
            "unbounded",
            "union",
            "unique",
            "update",
            "using",
            "values",
            "view",
            "when",
            "where",
            "window",
            "with",
            "within",
        ]],
        soft_keywords: &[],
        types: &[
            "bigint",
            "bigserial",
            "bool",
            "boolean",
            "bytea",
            "char",
            "date",
            "decimal",
            "double",
            "float",
            "int",
            "integer",
            "interval",
            "json",
            "jsonb",
            "numeric",
            "real",
            "serial",
            "smallint",
            "text",
            "time",
            "timestamp",
            "timestamptz",
            "uuid",
            "varchar",
        ],
        ignore_case: true,
        capitalized_types: false,
        macros: false,
        identifier_symbols: "",
    },
};

#[cfg(test)]
mod tests {
    use code_racer_engine::{corpus, graphemes};
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    use super::*;

    fn tokens_of(source: &str, language: CodeLanguage, needle: &str) -> Vec<Token> {
        let byte = source
            .find(needle)
            .unwrap_or_else(|| panic!("{needle:?} not in {source:?}"));
        let start = graphemes(&source[..byte]).len();
        let tokens = highlight(&graphemes(source), language);
        tokens[start..start + graphemes(needle).len()].to_vec()
    }

    fn token(source: &str, language: CodeLanguage, needle: &str) -> Token {
        let tokens = tokens_of(source, language, needle);
        assert!(
            tokens.windows(2).all(|pair| pair[0] == pair[1]),
            "{needle:?} in {source:?} is split: {tokens:?}"
        );
        tokens[0]
    }

    #[test]
    fn rust_words_are_classified() {
        let source = "pub fn parse(input: &str) -> Option<u32> { println!(\"{}\", input); x != y }";
        let rust = CodeLanguage::Rust;
        assert_eq!(token(source, rust, "pub"), Token::Keyword);
        assert_eq!(token(source, rust, "parse"), Token::Function);
        assert_eq!(token(source, rust, "input"), Token::Plain);
        assert_eq!(token(source, rust, "str"), Token::Type);
        assert_eq!(token(source, rust, "Option"), Token::Type);
        assert_eq!(token(source, rust, "u32"), Token::Type);
        assert_eq!(token(source, rust, "println!"), Token::Macro);
        assert_eq!(token(source, rust, "\"{}\""), Token::String);
        assert_eq!(token(source, rust, "x"), Token::Plain);
        assert_eq!(token(source, rust, "!="), Token::Punctuation);
        assert_eq!(token(source, rust, "->"), Token::Punctuation);
        assert_eq!(token("a!=b", rust, "a"), Token::Plain);
    }

    #[test]
    fn rust_lifetimes_are_not_char_literals() {
        let source = "fn f<'a>(x: &'a str) -> char { 'x' }";
        let rust = CodeLanguage::Rust;
        assert_eq!(token(source, rust, "'a"), Token::Keyword);
        assert_eq!(
            tokens_of(source, rust, "&'a str"),
            [
                Token::Punctuation,
                Token::Keyword,
                Token::Keyword,
                Token::Plain,
                Token::Type,
                Token::Type,
                Token::Type
            ]
        );
        assert_eq!(token(source, rust, "'x'"), Token::String);
        assert_eq!(token(source, rust, "}"), Token::Punctuation);
        assert_eq!(token("'outer: loop {}", rust, "'outer"), Token::Keyword);
    }

    #[test]
    fn rust_char_literals_handle_escapes_and_byte_prefixes() {
        let source = "let c = ['\\'', '\\n', '\\u{1F600}', b'x', ' ']; done";
        let rust = CodeLanguage::Rust;
        for literal in ["'\\''", "'\\n'", "'\\u{1F600}'", "b'x'", "' '"] {
            assert_eq!(token(source, rust, literal), Token::String, "{literal}");
        }
        assert_eq!(token(source, rust, "done"), Token::Plain);
    }

    #[test]
    fn rust_raw_strings_ignore_escapes_and_need_their_fence() {
        let source = r##"let p = r"C:\"; let q = r#"say "hi""#; let b = b"\x00"; end"##;
        let rust = CodeLanguage::Rust;
        assert_eq!(token(source, rust, r#"r"C:\""#), Token::String);
        assert_eq!(token(source, rust, r##"r#"say "hi""#"##), Token::String);
        assert_eq!(token(source, rust, r#"b"\x00""#), Token::String);
        assert_eq!(token(source, rust, "end"), Token::Plain);
    }

    #[test]
    fn rust_strings_may_span_lines() {
        let source = "let s = \"one\ntwo\"; x";
        assert_eq!(
            token(source, CodeLanguage::Rust, "\"one\ntwo\""),
            Token::String
        );
        assert_eq!(token(source, CodeLanguage::Rust, "x"), Token::Plain);
    }

    #[test]
    fn line_comments_stop_at_the_newline() {
        let source = "x // note: \"quoted\"\nlet y";
        let rust = CodeLanguage::Rust;
        assert_eq!(token(source, rust, "// note: \"quoted\""), Token::Comment);
        assert_eq!(token(source, rust, "\n"), Token::Plain);
        assert_eq!(token(source, rust, "let"), Token::Keyword);
    }

    #[test]
    fn block_comments_span_lines_and_may_be_unterminated() {
        let source = "a /* one\ntwo */ b";
        for language in [
            CodeLanguage::Rust,
            CodeLanguage::TypeScript,
            CodeLanguage::Sql,
        ] {
            assert_eq!(token(source, language, "/* one\ntwo */"), Token::Comment);
            assert_eq!(token(source, language, "b"), Token::Plain);
        }
        assert_eq!(
            token("a /* open", CodeLanguage::JavaScript, "/* open"),
            Token::Comment
        );
        assert_eq!(
            token(source, CodeLanguage::Python, "/*"),
            Token::Punctuation
        );
    }

    #[test]
    fn comment_markers_depend_on_the_language() {
        assert_eq!(
            token("# note", CodeLanguage::Python, "# note"),
            Token::Comment
        );
        assert_eq!(
            token("#[derive(Debug)]", CodeLanguage::Rust, "#"),
            Token::Punctuation
        );
        assert_eq!(
            token("#[derive(Debug)]", CodeLanguage::Rust, "derive"),
            Token::Function
        );
        assert_eq!(
            token("x-- -- note", CodeLanguage::Sql, "-- -- note"),
            Token::Comment
        );
        assert_eq!(
            token("x-- y", CodeLanguage::JavaScript, "--"),
            Token::Punctuation
        );
    }

    #[test]
    fn numbers_cover_common_literal_forms() {
        let source = "f(42, 3.14, 1_000, 0xff, 1e9, 2.5e-3, 10u32, 0..10, x1, 5.max(1))";
        let rust = CodeLanguage::Rust;
        for number in ["42", "3.14", "1_000", "0xff", "1e9", "2.5e-3", "10u32"] {
            assert_eq!(token(source, rust, number), Token::Number, "{number}");
        }
        assert_eq!(
            tokens_of(source, rust, "0..10"),
            [
                Token::Number,
                Token::Punctuation,
                Token::Punctuation,
                Token::Number,
                Token::Number
            ]
        );
        assert_eq!(token(source, rust, "x1"), Token::Plain);
        assert_eq!(token(source, rust, "max"), Token::Function);
        assert_eq!(token("0xe-1", rust, "-"), Token::Punctuation);
    }

    #[test]
    fn python_strings_prefixes_and_triple_quotes() {
        let source = "def greet(name: str) -> None:\n    \"\"\"Say \"hi\".\n    \"\"\"\n    return f\"hi {name}\" + rb'\\x' + len(name)  # done";
        let python = CodeLanguage::Python;
        assert_eq!(token(source, python, "def"), Token::Keyword);
        assert_eq!(token(source, python, "greet"), Token::Function);
        assert_eq!(token(source, python, "str"), Token::Type);
        assert_eq!(token(source, python, "None"), Token::Keyword);
        assert_eq!(
            token(source, python, "\"\"\"Say \"hi\".\n    \"\"\""),
            Token::String
        );
        assert_eq!(token(source, python, "f\"hi {name}\""), Token::String);
        assert_eq!(token(source, python, "rb'\\x'"), Token::String);
        assert_eq!(token(source, python, "len"), Token::Function);
        assert_eq!(token(source, python, "# done"), Token::Comment);
    }

    #[test]
    fn unterminated_single_line_strings_stop_at_the_newline() {
        let source = "x = 'oops\ny = 1";
        assert_eq!(token(source, CodeLanguage::Python, "'oops"), Token::String);
        assert_eq!(token(source, CodeLanguage::Python, "y"), Token::Plain);
        assert_eq!(token(source, CodeLanguage::Python, "1"), Token::Number);
        assert_eq!(token(source, CodeLanguage::JavaScript, "1"), Token::Number);
    }

    #[test]
    fn javascript_and_typescript_templates_and_types() {
        let source = "const $el: string = `a ${b}\nc` + 'd'; new Map()";
        for language in [CodeLanguage::JavaScript, CodeLanguage::TypeScript] {
            assert_eq!(token(source, language, "const"), Token::Keyword);
            assert_eq!(token(source, language, "$el"), Token::Plain);
            assert_eq!(token(source, language, "`a ${b}\nc`"), Token::String);
            assert_eq!(token(source, language, "'d'"), Token::String);
            assert_eq!(token(source, language, "Map"), Token::Type);
        }
        assert_eq!(
            token(source, CodeLanguage::TypeScript, "string"),
            Token::Type
        );
        assert_eq!(
            token(source, CodeLanguage::JavaScript, "string"),
            Token::Plain
        );
        assert_eq!(
            token("interface A {}", CodeLanguage::TypeScript, "interface"),
            Token::Keyword
        );
        assert_eq!(
            token("interface A {}", CodeLanguage::JavaScript, "interface"),
            Token::Plain
        );
    }

    #[test]
    fn sql_keywords_ignore_case_and_quotes_double_up() {
        let source =
            "Select count(*) FROM Users WHERE name = 'O''Brien' AND path = 'C:\\' -- note\nlimit 5";
        let sql = CodeLanguage::Sql;
        assert_eq!(token(source, sql, "Select"), Token::Keyword);
        assert_eq!(token(source, sql, "FROM"), Token::Keyword);
        assert_eq!(token(source, sql, "limit"), Token::Keyword);
        assert_eq!(token(source, sql, "count"), Token::Function);
        assert_eq!(token(source, sql, "Users"), Token::Plain);
        assert_eq!(token(source, sql, "'O''Brien'"), Token::String);
        assert_eq!(token(source, sql, "'C:\\'"), Token::String);
        assert_eq!(token(source, sql, "AND"), Token::Keyword);
        assert_eq!(token(source, sql, "-- note"), Token::Comment);
        assert_eq!(token(source, sql, "5"), Token::Number);
        assert_eq!(token("name VARCHAR(80)", sql, "VARCHAR"), Token::Type);
    }

    #[test]
    fn unicode_identifiers_and_symbols_are_plain() {
        let source = "let café = \"été\"; → 👍";
        let rust = CodeLanguage::Rust;
        assert_eq!(token(source, rust, "café"), Token::Plain);
        assert_eq!(token(source, rust, "\"été\""), Token::String);
        assert_eq!(token(source, rust, "→"), Token::Plain);
        assert_eq!(token(source, rust, "👍"), Token::Plain);
    }

    #[test]
    fn word_lists_are_sorted_for_binary_search() {
        for language in CodeLanguage::ALL {
            let words = &Syntax::of(language).words;
            let lists = words
                .keywords
                .iter()
                .chain([&words.soft_keywords, &words.types]);
            for list in lists {
                assert!(
                    list.windows(2).all(|pair| pair[0] < pair[1]),
                    "{language}: {list:?}"
                );
                if words.ignore_case {
                    assert!(
                        list.iter().all(|word| *word == word.to_lowercase()),
                        "{language}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_bundled_snippet_gets_one_token_per_grapheme() {
        for language in CodeLanguage::ALL {
            for snippet in corpus::snippets(language) {
                let graphemes = graphemes(snippet);
                let tokens = highlight(&graphemes, language);
                assert_eq!(tokens.len(), graphemes.len(), "{language}: {snippet}");
                assert!(tokens.contains(&Token::Keyword), "{language}: {snippet}");
            }
        }
    }

    #[test]
    fn number_suffixes_ending_in_e_are_not_exponents() {
        let rust = CodeLanguage::Rust;
        assert_eq!(
            tokens_of("n = 1usize-1;", rust, "1usize-1"),
            [
                [Token::Number; 6].as_slice(),
                &[Token::Punctuation, Token::Number]
            ]
            .concat()
        );
        assert_eq!(token("x = 2.5E+3;", rust, "2.5E+3"), Token::Number);
    }

    #[test]
    fn python_soft_keywords_only_open_statements() {
        let source = "match command.split():\n    case [\"go\", where]:\n        pass\n    case \"=\":\n        pass\nmatch = pattern.search(line)\ncase += 1\nprint(match, type(x))\ntype Point = tuple[int, int]";
        let python = CodeLanguage::Python;
        let tokens = highlight(&graphemes(source), python);
        let at = |needle: &str, nth: usize| {
            let byte = source.match_indices(needle).nth(nth).map(|(byte, _)| byte);
            let index = byte.map(|byte| graphemes(&source[..byte]).len());
            index.map(|index| tokens[index])
        };
        assert_eq!(at("match", 0), Some(Token::Keyword));
        assert_eq!(at("case", 0), Some(Token::Keyword));
        assert_eq!(at("case", 1), Some(Token::Keyword));
        assert_eq!(at("match", 1), Some(Token::Plain));
        assert_eq!(at("case", 2), Some(Token::Plain));
        assert_eq!(at("match", 2), Some(Token::Plain));
        assert_eq!(at("type", 0), Some(Token::Function));
        assert_eq!(at("type", 1), Some(Token::Keyword));
        assert_eq!(
            token("match x:", CodeLanguage::Rust, "match"),
            Token::Keyword
        );
        assert_eq!(
            token("const match = 1", CodeLanguage::JavaScript, "match"),
            Token::Plain
        );
    }

    #[test]
    fn sql_procedural_keywords_are_recognised() {
        let source = "CREATE OR REPLACE FUNCTION touch() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN NEW.at := NOW(); RETURN NEW; END $$";
        let sql = CodeLanguage::Sql;
        for keyword in [
            "REPLACE", "FUNCTION", "RETURNS", "TRIGGER", "LANGUAGE", "RETURN", "NEW",
        ] {
            assert_eq!(token(source, sql, keyword), Token::Keyword, "{keyword}");
        }
        assert_eq!(token(source, sql, "touch"), Token::Function);
        assert_eq!(token(source, sql, "plpgsql"), Token::Plain);
    }

    const PIECES: &[&str] = &[
        "\"", "'", "`", "\\", "/", "*", "/*", "*/", "//", "#", "-", "--", "r", "b", "f", "r#\"",
        "\"#", "\"\"\"", "'a", "0x", "1", "1e", "1usize", ".", "+", "=", "!", "(", "\n", " ", "é",
        "e\u{301}", "👍🏽", "$x", "fn", "match ", "case", "Type", "x1", "SELECT",
    ];

    fn random_source(rng: &mut StdRng, pieces: &[&str], max_len: usize) -> String {
        let len = rng.random_range(0..max_len);
        (0..len)
            .map(|_| pieces[rng.random_range(0..pieces.len())])
            .collect()
    }

    fn random_char(rng: &mut StdRng) -> char {
        let ranges = [
            0..0x80,
            0x80..0x800,
            0x300..0x370,
            0x1F300..0x1FAFF,
            0..0x11_0000,
        ];
        let range = ranges[rng.random_range(0..ranges.len())].clone();
        char::from_u32(rng.random_range(range)).unwrap_or('\u{FFFD}')
    }

    #[test]
    fn arbitrary_input_never_panics_and_keeps_one_token_per_grapheme() {
        let mut rng = StdRng::seed_from_u64(42);
        for round in 0..4_000 {
            let source = if round % 2 == 0 {
                random_source(&mut rng, PIECES, 30)
            } else {
                (0..rng.random_range(0..30))
                    .map(|_| random_char(&mut rng))
                    .collect()
            };
            let graphemes = graphemes(&source);
            for language in CodeLanguage::ALL {
                assert_eq!(
                    highlight(&graphemes, language).len(),
                    graphemes.len(),
                    "{source:?}"
                );
            }
        }
    }

    #[test]
    fn whitespace_is_never_highlighted_as_code() {
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..2_000 {
            let graphemes = graphemes(&random_source(&mut rng, PIECES, 30));
            for language in CodeLanguage::ALL {
                let tokens = highlight(&graphemes, language);
                for (grapheme, token) in graphemes.iter().zip(&tokens) {
                    let blank = grapheme == " " || grapheme == "\n";
                    assert!(
                        !blank || matches!(token, Token::Plain | Token::String | Token::Comment),
                        "{language}: {:?} gives {token:?}",
                        graphemes.concat()
                    );
                }
            }
        }
    }

    const BODY: &[&str] = &[
        "a", "Z", "7", " ", "é", "👍🏽", "#", "//", "--", "/*", "*/", "$", "{", "}", "(", "x1",
    ];
    const ESCAPES: &[&str] = &["\\\\", "\\n", "\\\"", "\\'", "\\`", "\\\n"];

    fn body(rng: &mut StdRng, extra: &[&[&str]]) -> String {
        let pieces: Vec<&str> = extra.iter().fold(BODY.to_vec(), |mut all, more| {
            all.extend_from_slice(more);
            all
        });
        random_source(rng, &pieces, 12)
    }

    fn rust_literal(rng: &mut StdRng) -> String {
        match rng.random_range(0..4) {
            0 => format!("\"{}\"", body(rng, &[ESCAPES, &["'", "\n"]])),
            1 => {
                let raw: Vec<&str> = BODY.iter().copied().filter(|piece| *piece != "#").collect();
                format!("r#\"{}\"#", body(rng, &[&raw, &["\"", "\\"]]))
            }
            2 => format!("b\"{}\"", body(rng, &[ESCAPES])),
            _ => [
                "'a'",
                "'\\''",
                "'\\\\'",
                "'\\n'",
                "'é'",
                "'👍🏽'",
                "b'x'",
                "'\"'",
            ][rng.random_range(0..8)]
            .to_owned(),
        }
    }

    fn python_literal(rng: &mut StdRng) -> String {
        let prefix = ["", "f", "r", "b", "rb", "F", "Rb", "u"][rng.random_range(0..8)];
        let literal = match rng.random_range(0..3) {
            0 => format!("\"{}\"", body(rng, &[ESCAPES, &["'"]])),
            1 => format!("'{}'", body(rng, &[ESCAPES, &["\""]])),
            _ => format!("\"\"\"{}\"\"\"", body(rng, &[ESCAPES, &["'", "\n"]])),
        };
        format!("{prefix}{literal}")
    }

    fn javascript_literal(rng: &mut StdRng) -> String {
        match rng.random_range(0..3) {
            0 => format!("\"{}\"", body(rng, &[ESCAPES, &["'", "`"]])),
            1 => format!("'{}'", body(rng, &[ESCAPES, &["\"", "`"]])),
            _ => format!("`{}`", body(rng, &[ESCAPES, &["'", "\"", "\n", "${x}"]])),
        }
    }

    fn sql_literal(rng: &mut StdRng) -> String {
        match rng.random_range(0..2) {
            0 => format!("'{}'", body(rng, &[&["''", "\"", "\\", "\n"]])),
            _ => format!("\"{}\"", body(rng, &[&["'", "\n"]])),
        }
    }

    fn string_literal(rng: &mut StdRng, language: CodeLanguage) -> String {
        match language {
            CodeLanguage::Rust => rust_literal(rng),
            CodeLanguage::Python => python_literal(rng),
            CodeLanguage::TypeScript | CodeLanguage::JavaScript => javascript_literal(rng),
            CodeLanguage::Sql => sql_literal(rng),
        }
    }

    fn comment(rng: &mut StdRng, language: CodeLanguage) -> String {
        let syntax = Syntax::of(language);
        if syntax.block_comments && rng.random_bool(0.5) {
            let plain: Vec<&str> = BODY
                .iter()
                .copied()
                .filter(|piece| !piece.contains(['*', '/']))
                .collect();
            format!(
                "/*{}*/",
                random_source(rng, &[&plain[..], &["\n", "*"]].concat(), 12)
            )
        } else {
            format!("{}{}", syntax.line_comment, body(rng, &[]))
        }
    }

    fn number(rng: &mut StdRng) -> String {
        let digits = |rng: &mut StdRng| random_source(rng, &["0", "7", "9", "1_0"], 4) + "1";
        let integer = digits(rng);
        match rng.random_range(0..4) {
            0 => integer,
            1 => format!("{integer}.{}", digits(rng)),
            2 => {
                let sign = ["", "+", "-"][rng.random_range(0..3)];
                let marker = ["e", "E"][rng.random_range(0..2)];
                format!("{integer}{marker}{sign}{}", digits(rng))
            }
            _ => format!("0x{}", random_source(rng, &["f", "F", "0", "a9"], 4) + "e"),
        }
    }

    fn assert_lexeme_is_closed(lexeme: &str, token: Token, rest: &str, language: CodeLanguage) {
        let source = format!("{lexeme}\n{rest}");
        let mut expected = vec![token; graphemes(lexeme).len()];
        expected.push(Token::Plain);
        expected.extend(highlight(&graphemes(rest), language));
        assert_eq!(
            highlight(&graphemes(&source), language),
            expected,
            "{language}: {source:?}"
        );
    }

    #[test]
    fn strings_comments_and_numbers_end_where_they_should() {
        let mut rng = StdRng::seed_from_u64(11);
        for _ in 0..1_500 {
            for language in CodeLanguage::ALL {
                let rest = random_source(&mut rng, PIECES, 12);
                let literal = string_literal(&mut rng, language);
                assert_lexeme_is_closed(&literal, Token::String, &rest, language);
                let comment = comment(&mut rng, language);
                assert_lexeme_is_closed(&comment, Token::Comment, &rest, language);
                let number = number(&mut rng);
                assert_lexeme_is_closed(&number, Token::Number, &rest, language);
            }
        }
    }

    #[test]
    fn unterminated_single_line_strings_end_with_their_line() {
        let mut rng = StdRng::seed_from_u64(13);
        let languages = [
            CodeLanguage::Python,
            CodeLanguage::TypeScript,
            CodeLanguage::JavaScript,
        ];
        for _ in 0..1_000 {
            for language in languages {
                let rest = random_source(&mut rng, PIECES, 12);
                let quote = ["\"", "'"][rng.random_range(0..2)];
                let open = format!("{quote}{}", body(&mut rng, &[ESCAPES]));
                assert_lexeme_is_closed(&open, Token::String, &rest, language);
            }
        }
    }

    #[test]
    fn digits_inside_identifiers_are_never_numbers() {
        let mut rng = StdRng::seed_from_u64(5);
        for _ in 0..1_000 {
            let identifier = format!(
                "x{}",
                random_source(&mut rng, &["1", "a", "_", "9e", "0x"], 8)
            );
            for language in CodeLanguage::ALL {
                let tokens = highlight(&graphemes(&identifier), language);
                assert!(!tokens.contains(&Token::Number), "{language}: {identifier}");
            }
        }
    }

    #[test]
    fn sql_highlighting_ignores_ascii_case() {
        let mut rng = StdRng::seed_from_u64(9);
        let ascii: Vec<&str> = PIECES
            .iter()
            .copied()
            .filter(|piece| piece.is_ascii())
            .chain(["select", "From", "varchar", "Count(", "nulL", "0XfF", "1E5"])
            .collect();
        for _ in 0..2_000 {
            let source = random_source(&mut rng, &ascii, 20);
            let upper = highlight(&graphemes(&source.to_ascii_uppercase()), CodeLanguage::Sql);
            let lower = highlight(&graphemes(&source.to_ascii_lowercase()), CodeLanguage::Sql);
            assert_eq!(upper, lower, "{source:?}");
        }
    }
}
