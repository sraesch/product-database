use std::{io::Read, path::PathBuf};

use log::info;
use product_db::PostgresConfig;
use serde::Deserialize;

use crate::logging::LogLevel;

use anyhow::{Context, Result};

/// The program options of the CLI.
pub struct ProgramOptions {
    /// The path to the configuration file.
    pub config_path: PathBuf,
}

/// The configuration for the product-db-cli program.
#[derive(Debug, Deserialize)]
pub struct ProgramConfig {
    pub log: LogLevel,
    /// The address where to expose the controller REST API.
    pub address: String,
    /// The Postgres config.
    pub postgres: PostgresConfig,
}

impl ProgramConfig {
    pub fn print_to_log(&self) {
        info!("Configuration:");
        info!("Log level: {}", self.log);
        info!("Postgres:");
        info!("Postgres Host: {}", self.postgres.host);
        info!("Postgres Port: {}", self.postgres.port);
        info!("Postgres User: {}", self.postgres.user);
        info!("Postgres Password: {}", self.postgres.password);
        info!("Postgres Database: {}", self.postgres.dbname);
    }

    pub fn from_reader<R: Read>(r: R) -> Result<Self> {
        let mut s = String::new();

        let mut r = r;
        r.read_to_string(&mut s)?;

        let config: Self = toml::from_str(&s)?;

        Ok(config)
    }
}

impl TryFrom<ProgramOptions> for ProgramConfig {
    type Error = anyhow::Error;

    fn try_from(value: ProgramOptions) -> Result<Self, Self::Error> {
        let config_path = value.config_path.as_path();
        let r = std::fs::File::open(config_path)
            .with_context(|| format!("Failed to open file {}", config_path.display()))?;
        ProgramConfig::from_reader(r)
    }
}

#[cfg(test)]
mod test {
    use crate::logging::LogLevel;

    use super::ProgramConfig;

    #[test]
    fn test_loading_config() {
        let data = include_bytes!("../../example/config.toml");
        let c = ProgramConfig::from_reader(data.as_slice()).unwrap();

        assert_eq!(c.log, LogLevel::Info);

        assert_eq!(c.postgres.dbname, "product_db");
        assert_eq!(c.postgres.host, "localhost");
        assert_eq!(c.postgres.port, 5432);
        assert_eq!(c.postgres.user, "postgres");
        assert_eq!(c.postgres.password.secret(), "postgres");
    }
}
