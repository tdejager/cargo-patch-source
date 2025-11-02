use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum PatchError {
    #[error("Failed to read Cargo.toml at {path}")]
    #[diagnostic(code(patch::io::read))]
    CargoTomlReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write Cargo.toml at {path}")]
    #[diagnostic(code(patch::io::write))]
    CargoTomlWriteError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse Cargo.toml at {path}")]
    #[diagnostic(code(patch::toml::parse))]
    TomlParseError {
        path: PathBuf,
        #[source]
        source: toml_edit::TomlError,
    },

    #[error("Failed to query cargo metadata")]
    #[diagnostic(code(patch::cargo::metadata))]
    CargoMetadataError {
        #[source]
        source: cargo_metadata::Error,
    },

    #[error("No source specified. Use --path or --git")]
    #[diagnostic(code(patch::cli::no_source))]
    NoSourceSpecified,

    #[error("Source path does not exist: {path}")]
    #[diagnostic(code(patch::source::not_found))]
    SourceNotFound { path: PathBuf },

    #[error("Source path is not a valid cargo workspace: {path}")]
    #[diagnostic(code(patch::source::not_workspace))]
    NotAWorkspace { path: PathBuf },

    #[error("No crates found matching pattern: {pattern}")]
    #[diagnostic(code(patch::pattern::no_match))]
    NoMatchingCrates { pattern: String },

    #[error("No patches found to remove")]
    #[diagnostic(code(patch::remove::not_found))]
    NoPatchesFound,

    #[error("Failed to parse pattern: {pattern}")]
    #[diagnostic(code(patch::pattern::invalid))]
    InvalidPattern {
        pattern: String,
        #[source]
        source: regex::Error,
    },

    #[error("Failed to serialize/deserialize JSON")]
    #[diagnostic(code(patch::json::error))]
    JsonError {
        #[source]
        source: serde_json::Error,
    },
}

pub type Result<T> = std::result::Result<T, PatchError>;
