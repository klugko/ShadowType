use rand::SeedableRng;

use super::*;

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
