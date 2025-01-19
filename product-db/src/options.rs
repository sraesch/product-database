use crate::PostgresConfig;

/// The options for running the product database.
#[derive(Debug, Clone)]
pub struct Options {
    /// The address where to expose the controller REST API.
    pub address: String,
    /// The Postgres config.
    pub postgres: PostgresConfig,
}
