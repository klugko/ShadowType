//! Natural languages and programming languages that texts can be generated in.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A natural language with a bundled word list and quote collection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    #[serde(alias = "en")]
    English,
    #[serde(alias = "fr")]
    French,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::French];

    pub const fn name(self) -> &'static str {
        match self {
            Self::English => "english",
            Self::French => "french",
        }
    }

    const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::English => &["en", "eng"],
            Self::French => &["fr", "francais", "français"],
        }
    }

    /// French typography puts a space before `? ! ; :`.
    pub(crate) const fn spaces_high_punctuation(self) -> bool {
        matches!(self, Self::French)
    }
}

/// A programming language with a bundled collection of code snippets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeLanguage {
    #[default]
    #[serde(alias = "rs")]
    Rust,
    #[serde(alias = "py")]
    Python,
    #[serde(alias = "ts")]
    TypeScript,
    #[serde(alias = "js")]
    JavaScript,
    Sql,
}

impl CodeLanguage {
    pub const ALL: [Self; 5] = [
        Self::Rust,
        Self::Python,
        Self::TypeScript,
        Self::JavaScript,
        Self::Sql,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Sql => "sql",
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Python => "py",
            Self::TypeScript => "ts",
            Self::JavaScript => "js",
            Self::Sql => "sql",
        }
    }

    /// Guesses the language of a source file from its extension.
    pub fn from_extension(extension: &str) -> Option<Self> {
        let extension = extension.to_ascii_lowercase();
        match extension.as_str() {
            "tsx" => Some(Self::TypeScript),
            "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            _ => Self::ALL
                .into_iter()
                .find(|language| language.extension() == extension),
        }
    }

    const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::Python => &["py"],
            Self::TypeScript => &["ts"],
            Self::JavaScript => &["js"],
            Self::Sql => &["postgres", "postgresql"],
        }
    }
}

/// Returned when a language name is not recognised.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown language `{0}`")]
pub struct UnknownLanguage(String);

macro_rules! impl_language_traits {
    ($language:ty) => {
        impl fmt::Display for $language {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.name())
            }
        }

        impl FromStr for $language {
            type Err = UnknownLanguage;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                let wanted = input.trim().to_lowercase();
                <$language>::ALL
                    .into_iter()
                    .find(|language| {
                        language.name() == wanted || language.aliases().contains(&wanted.as_str())
                    })
                    .ok_or_else(|| UnknownLanguage(input.to_owned()))
            }
        }
    };
}

impl_language_traits!(Language);
impl_language_traits!(CodeLanguage);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_through_from_str() {
        for language in Language::ALL {
            assert_eq!(language.name().parse::<Language>(), Ok(language));
        }
        for language in CodeLanguage::ALL {
            assert_eq!(language.name().parse::<CodeLanguage>(), Ok(language));
        }
    }

    #[test]
    fn aliases_and_case_are_accepted() {
        assert_eq!("FR".parse::<Language>(), Ok(Language::French));
        assert_eq!(" English ".parse::<Language>(), Ok(Language::English));
        assert_eq!("ts".parse::<CodeLanguage>(), Ok(CodeLanguage::TypeScript));
        assert!("klingon".parse::<Language>().is_err());
    }

    #[test]
    fn serde_uses_lowercase_names() {
        let json = serde_json::to_string(&CodeLanguage::TypeScript).expect("serialize");
        assert_eq!(json, "\"typescript\"");
        let parsed: Language = serde_json::from_str("\"french\"").expect("deserialize");
        assert_eq!(parsed, Language::French);
        let alias: CodeLanguage = serde_json::from_str("\"ts\"").expect("alias");
        assert_eq!(alias, CodeLanguage::TypeScript);
    }

    #[test]
    fn extensions_map_back_to_languages() {
        assert_eq!(CodeLanguage::from_extension("RS"), Some(CodeLanguage::Rust));
        assert_eq!(
            CodeLanguage::from_extension("tsx"),
            Some(CodeLanguage::TypeScript)
        );
        assert_eq!(CodeLanguage::from_extension("md"), None);
    }
}
