use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "&'static str")]
pub enum Language {
    #[default]
    English,
    French,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::French];

    /// Every accepted spelling, ignoring case, the canonical name first.
    const fn names(self) -> &'static [&'static str] {
        match self {
            Self::English => &["english", "en", "eng"],
            Self::French => &["french", "fr", "francais", "français"],
        }
    }

    /// French typography puts a space before `? ! ; :`.
    pub(crate) const fn spaces_high_punctuation(self) -> bool {
        matches!(self, Self::French)
    }

    pub(crate) const fn decimal_separator(self) -> char {
        match self {
            Self::English => '.',
            Self::French => ',',
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "&'static str")]
pub enum CodeLanguage {
    #[default]
    Rust,
    Python,
    TypeScript,
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

    /// Every accepted spelling, ignoring case, the canonical name first.
    const fn names(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rust", "rs"],
            Self::Python => &["python", "py"],
            Self::TypeScript => &["typescript", "ts"],
            Self::JavaScript => &["javascript", "js"],
            Self::Sql => &["sql", "postgres", "postgresql"],
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
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown language `{name}`, expected one of {expected}")]
pub struct UnknownLanguage {
    name: String,
    expected: String,
}

/**
 * Names, display, parsing and serde all go through the `names` table of a
 * language enum, so the command line, the configuration file and the wire
 * accept exactly the same spellings.
 */
macro_rules! impl_language_traits {
    ($language:ty) => {
        impl $language {
            /// Canonical lowercase name, used for display and when saving.
            pub const fn name(self) -> &'static str {
                self.names()[0]
            }
        }

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
                    .find(|language| language.names().contains(&wanted.as_str()))
                    .ok_or_else(|| UnknownLanguage {
                        name: input.to_owned(),
                        expected: <$language>::ALL.map(<$language>::name).join(", "),
                    })
            }
        }

        impl TryFrom<String> for $language {
            type Error = UnknownLanguage;

            fn try_from(name: String) -> Result<Self, Self::Error> {
                name.parse()
            }
        }

        impl From<$language> for &'static str {
            fn from(language: $language) -> Self {
                language.name()
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

    fn spellings(name: &str) -> [String; 3] {
        let mut chars = name.chars();
        let capitalized = chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default();
        [name.to_owned(), name.to_uppercase(), capitalized]
    }

    fn assert_serde_agrees_with_from_str<L>(all: &[L], names: fn(L) -> &'static [&'static str])
    where
        L: Copy + fmt::Debug + PartialEq + FromStr + serde::de::DeserializeOwned,
    {
        for &language in all {
            for spelling in names(language).iter().flat_map(|name| spellings(name)) {
                let json = format!("\"{spelling}\"");
                let deserialized = serde_json::from_str::<L>(&json).ok();
                assert_eq!(deserialized, Some(language), "{spelling}");
                assert_eq!(spelling.parse::<L>().ok(), Some(language), "{spelling}");
            }
        }
    }

    #[test]
    fn serde_accepts_every_name_from_str_accepts() {
        assert_serde_agrees_with_from_str(&Language::ALL, Language::names);
        assert_serde_agrees_with_from_str(&CodeLanguage::ALL, CodeLanguage::names);
    }

    #[test]
    fn hand_written_config_names_deserialize() {
        for (json, expected) in [
            ("\"French\"", Language::French),
            ("\"français\"", Language::French),
        ] {
            assert_eq!(serde_json::from_str::<Language>(json).ok(), Some(expected));
        }
        for json in ["\"postgres\"", "\"PostgreSQL\""] {
            assert_eq!(
                serde_json::from_str::<CodeLanguage>(json).ok(),
                Some(CodeLanguage::Sql)
            );
        }
        assert!(serde_json::from_str::<Language>("\"klingon\"").is_err());
    }

    #[test]
    fn unknown_names_list_the_valid_ones() {
        let error = "klingon"
            .parse::<Language>()
            .err()
            .map(|error| error.to_string());
        assert_eq!(
            error.as_deref(),
            Some("unknown language `klingon`, expected one of english, french")
        );
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
