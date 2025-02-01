use serde_yaml::Error as YamlError;
use thiserror::Error;

use crate::SortingField;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed parsing the config: {0}")]
    ParsingConfigError(#[from] Box<YamlError>),

    #[error("Failed loading the config: {0}")]
    ConfigError(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] Box<serde_json::Error>),

    #[error("Invalid config: {0}")]
    InvalidConfigError(String),

    #[error("Invalid sorting: {0} is not supported")]
    InvalidSortingError(SortingField),

    #[error("Network error: {0}")]
    NetworkError(#[from] tokio::io::Error),

    #[error("IO Error: {0}")]
    IO(#[from] Box<std::io::Error>),

    #[error("SQLx DB error: {0}")]
    DBError(#[from] Box<sqlx::Error>),

    #[error("Internal error: {0}")]
    InternalError(String),
}

/// The result type used in this crate.
pub type Result<T> = std::result::Result<T, Error>;
