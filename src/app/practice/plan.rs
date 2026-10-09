use code_racer_engine::{CodeLanguage, Language, TextSource, WordOptions};

use super::CustomText;
use crate::{
    app::Disguise,
    config::{Look, Practice},
    history::Record,
};

/// What a solo session is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Text(TextSource),
    Timed {
        language: Language,
        options: WordOptions,
        seconds: u16,
    },
    File(CustomText),
}

impl Plan {
    pub fn from_practice(practice: &Practice) -> Self {
        practice.text_source().map_or(
            Self::Timed {
                language: practice.language,
                options: practice.word_options(),
                seconds: practice.duration,
            },
            Self::Text,
        )
    }

    /**
     * File name shown in the editor for this session, the name of the
     * file prose is disguised as in a look other than notes.
     */
    pub fn title(&self, disguise: Disguise<'_>) -> String {
        match self {
            Self::Text(TextSource::Code { language }) => code_file_name(*language),
            Self::File(custom) => custom.name.clone(),
            _ if disguise.look != Look::Notes => disguised_name(disguise),
            Self::Text(TextSource::Words { .. }) => "notes.md".to_owned(),
            Self::Timed { .. } => "scratch.txt".to_owned(),
            Self::Text(TextSource::Quote { .. }) => "README.md".to_owned(),
        }
    }

    /// Short description such as `words 50 · english · punctuation`.
    pub fn label(&self) -> String {
        self.label_parts().join(" · ")
    }

    /// The parts of [`Plan::label`], from the most telling to the least.
    pub fn label_parts(&self) -> Vec<String> {
        let mut parts = vec![self.mode_label(), self.language_label()];
        if let Self::Text(TextSource::Words { options, .. }) | Self::Timed { options, .. } = self {
            if options.punctuation {
                parts.push("punctuation".to_owned());
            }
            if options.numbers {
                parts.push("numbers".to_owned());
            }
        }
        parts
    }

    /**
     * Whether the text is prose, which a look can disguise, rather than
     * code or a file of the player's.
     */
    pub fn is_prose(&self) -> bool {
        matches!(
            self,
            Self::Timed { .. } | Self::Text(TextSource::Words { .. } | TextSource::Quote { .. })
        )
    }

    pub fn syntax(&self) -> Option<CodeLanguage> {
        match self {
            Self::Text(TextSource::Code { language }) => Some(*language),
            Self::File(custom) => custom.language,
            _ => None,
        }
    }

    /// What was practised, as [`Record::mode`] keeps it.
    pub fn mode_label(&self) -> String {
        match self {
            Self::Text(TextSource::Words { count, .. }) => format!("words {count}"),
            Self::Timed { seconds, .. } => format!("time {seconds}"),
            Self::Text(TextSource::Quote { .. }) => "quote".to_owned(),
            Self::Text(TextSource::Code { .. }) => Record::CODE_MODE.to_owned(),
            Self::File(_) => "file".to_owned(),
        }
    }

    pub fn language_label(&self) -> String {
        match self {
            Self::Text(TextSource::Words { language, .. } | TextSource::Quote { language })
            | Self::Timed { language, .. } => language.to_string(),
            Self::Text(TextSource::Code { language }) => language.to_string(),
            Self::File(custom) => custom
                .language
                .map_or_else(|| "text".to_owned(), |language| language.to_string()),
        }
    }
}

/// The name of the file prose is typed in, in a look other than notes.
pub fn disguised_name(disguise: Disguise<'_>) -> String {
    match disguise.look {
        Look::Todo => "TODO.md".to_owned(),
        Look::Commit => "COMMIT_EDITMSG".to_owned(),
        Look::Docs => {
            let stem = match disguise.language {
                CodeLanguage::Rust => "lib",
                CodeLanguage::Python => "utils",
                CodeLanguage::TypeScript => "api",
                CodeLanguage::JavaScript => "helpers",
                CodeLanguage::Sql => "schema",
            };
            format!("{stem}.{}", disguise.language.extension())
        }
        Look::Log => "server.log".to_owned(),
        Look::Mail => "draft.eml".to_owned(),
        Look::Notes | Look::Shuffle => "notes.md".to_owned(),
    }
}

pub fn code_file_name(language: CodeLanguage) -> String {
    let stem = match language {
        CodeLanguage::Rust => "main",
        CodeLanguage::Python => "app",
        CodeLanguage::TypeScript | CodeLanguage::JavaScript => "index",
        CodeLanguage::Sql => "query",
    };
    format!("{stem}.{}", language.extension())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Mode;

    #[test]
    fn plan_reflects_the_settings() {
        let mut settings = Practice {
            mode: Mode::Words,
            word_count: 25,
            punctuation: true,
            ..Practice::default()
        };
        let plan = Plan::from_practice(&settings);
        assert_eq!(plan.label(), "words 25 · english · punctuation");
        assert_eq!(plan.title(Disguise::default()), "notes.md");
        let commit = Disguise {
            look: Look::Commit,
            ..Disguise::default()
        };
        assert_eq!(plan.title(commit), "COMMIT_EDITMSG");
        settings.mode = Mode::Code;
        settings.code_language = CodeLanguage::Sql;
        assert_eq!(
            Plan::from_practice(&settings).title(commit),
            "query.sql",
            "code looks like code"
        );
        assert_eq!(
            Plan::from_practice(&settings).syntax(),
            Some(CodeLanguage::Sql)
        );
    }
}
