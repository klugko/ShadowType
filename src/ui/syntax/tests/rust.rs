use super::*;

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
