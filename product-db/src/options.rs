use serde::Deserialize;

use crate::PostgresConfig;

/// The options for running the product database.
#[derive(Debug, Clone)]
pub struct Options {
    /// The options for the REST endpoint.
    pub endpoint: EndpointOptions,
    /// The Postgres config.
    pub postgres: PostgresConfig,
}

/// The options for the endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct EndpointOptions {
    /// The address to bind the endpoint to.
    pub address: String,

    /// The allowed origin for CORS requests.
    pub allow_origin: String,
}

impl Default for EndpointOptions {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:8080".to_string(),
            allow_origin: "*".to_string(),
        }
    }
}
