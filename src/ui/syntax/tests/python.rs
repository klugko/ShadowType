use super::*;

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
