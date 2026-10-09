use rand::SeedableRng;

use super::*;

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
fn unterminated_single_line_strings_stop_at_the_newline() {
    let source = "x = 'oops\ny = 1";
    assert_eq!(token(source, CodeLanguage::Python, "'oops"), Token::String);
    assert_eq!(token(source, CodeLanguage::Python, "y"), Token::Plain);
    assert_eq!(token(source, CodeLanguage::Python, "1"), Token::Number);
    assert_eq!(token(source, CodeLanguage::JavaScript, "1"), Token::Number);
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
fn unicode_identifiers_and_symbols_are_plain() {
    let source = "let café = \"été\"; → 👍";
    let rust = CodeLanguage::Rust;
    assert_eq!(token(source, rust, "café"), Token::Plain);
    assert_eq!(token(source, rust, "\"été\""), Token::String);
    assert_eq!(token(source, rust, "→"), Token::Plain);
    assert_eq!(token(source, rust, "👍"), Token::Plain);
}
