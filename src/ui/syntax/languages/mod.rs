mod javascript;
mod python;
mod rust;
mod sql;

use code_racer_engine::CodeLanguage;

use super::{Quote, Syntax};

const DOUBLE_QUOTE: Quote = Quote {
    mark: "\"",
    multiline: false,
};
const SINGLE_QUOTE: Quote = Quote {
    mark: "'",
    multiline: false,
};

impl Syntax {
    pub(super) fn of(language: CodeLanguage) -> &'static Self {
        match language {
            CodeLanguage::Rust => &rust::RUST,
            CodeLanguage::Python => &python::PYTHON,
            CodeLanguage::TypeScript => &javascript::TYPESCRIPT,
            CodeLanguage::JavaScript => &javascript::JAVASCRIPT,
            CodeLanguage::Sql => &sql::SQL,
        }
    }
}
