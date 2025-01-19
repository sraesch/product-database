use deadpool_postgres::{CreatePoolError, PoolError};
use serde_yaml::Error as YamlError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("There has been no message in the queue.")]
    MessageQueueNoMessage,

    #[error("Failed parsing the config: {0}")]
    ParsingConfigError(#[from] Box<YamlError>),

    #[error("Serialization error: {0}")]
    Serialization(#[from] Box<serde_json::Error>),

    #[error("Invalid config: {0}")]
    InvalidConfigError(String),

    #[error("IO Error: {0}")]
    IO(#[from] Box<std::io::Error>),

    #[error("Failed to initialize connection pool: {0}")]
    DBCreatePoolError(#[from] Box<CreatePoolError>),

    #[error("Failed to get connection: {0}")]
    DBPoolError(#[from] Box<PoolError>),

    #[error("Postgres DB error: {0}")]
    DBError(#[from] Box<tokio_postgres::Error>),

    #[error("Internal error: {0}")]
    InternalError(String),
}

/// The result type used in this crate.
pub type Result<T> = std::result::Result<T, Error>;
