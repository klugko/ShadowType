use code_racer_engine::{CodeLanguage, graphemes};
use ratatui::text::Span;

use crate::ui::{editor::Row, syntax, theme::Palette};

pub(super) fn comment_marker(language: CodeLanguage) -> &'static str {
    match language {
        CodeLanguage::Rust => "/// ",
        CodeLanguage::Python => "    ",
        CodeLanguage::TypeScript | CodeLanguage::JavaScript => " * ",
        CodeLanguage::Sql => "-- ",
    }
}

pub(super) fn header(language: CodeLanguage, palette: &Palette) -> Vec<Row> {
    code(header_lines(language), language, palette)
}

pub(super) fn footer(language: CodeLanguage, palette: &Palette) -> Vec<Row> {
    code(footer_lines(language), language, palette)
}

fn header_lines(language: CodeLanguage) -> &'static [&'static str] {
    match language {
        CodeLanguage::Rust | CodeLanguage::Sql => &[],
        CodeLanguage::Python => &["def reconcile(state: State) -> None:", "    \"\"\""],
        CodeLanguage::TypeScript | CodeLanguage::JavaScript => &["/**"],
    }
}

fn footer_lines(language: CodeLanguage) -> &'static [&'static str] {
    match language {
        CodeLanguage::Rust => &[
            "pub fn reconcile(state: &mut State) -> Result<(), Error> {",
            "    todo!()",
            "}",
        ],
        CodeLanguage::Python => &["    \"\"\"", "    raise NotImplementedError"],
        CodeLanguage::TypeScript => &[
            " */",
            "export function reconcile(state: State): void {",
            "  throw new Error(\"not implemented\");",
            "}",
        ],
        CodeLanguage::JavaScript => &[
            " */",
            "export function reconcile(state) {",
            "  throw new Error(\"not implemented\");",
            "}",
        ],
        CodeLanguage::Sql => &[
            "CREATE VIEW active_accounts AS",
            "SELECT id, email FROM accounts WHERE closed_at IS NULL;",
        ],
    }
}

fn code(lines: &[&str], language: CodeLanguage, palette: &Palette) -> Vec<Row> {
    lines
        .iter()
        .map(|line| code_line(line, language, palette))
        .collect()
}

fn code_line(line: &str, language: CodeLanguage, palette: &Palette) -> Row {
    let text = graphemes(line);
    let tokens = syntax::highlight(&text, language);
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (grapheme, token) in text.iter().zip(tokens) {
        let style = palette.syntax(token);
        match spans.last_mut() {
            Some(last) if last.style == style => last.content.to_mut().push_str(grapheme),
            _ => spans.push(Span::styled(grapheme.clone(), style)),
        }
    }
    Row::new(spans)
}
