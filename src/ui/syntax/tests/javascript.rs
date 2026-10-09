use super::*;

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
