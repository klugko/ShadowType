//! Solo practice: the settings form and the session being typed.

mod file;
mod form;
mod plan;
mod run;

use std::io;

use code_racer_engine::CodeLanguage;
use thiserror::Error;

pub use form::{Field, adjust, fields, row};
pub use plan::{Plan, code_file_name, disguised_name};
pub use run::{SoloResult, SoloRun};

/// The content of a file chosen with `:edit` or `--file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomText {
    pub name: String,
    pub text: String,
    pub language: Option<CodeLanguage>,
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("cannot read {path}: {source}")]
    Unreadable { path: String, source: io::Error },
    #[error("{0} is not a regular file")]
    NotAFile(String),
    #[error("{0} is not a text file")]
    Binary(String),
    #[error("{0} is empty")]
    Empty(String),
}
