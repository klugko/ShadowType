use rand::SeedableRng;

use super::*;

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
