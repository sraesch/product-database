use std::path::PathBuf;

use anyhow::Result;
use clap::{arg, value_parser, Command};
use log::{error, info, LevelFilter};
use logging::initialize_logging;
use options::{ProgramConfig, ProgramOptions};
use product_db::Options;

mod logging;
mod options;

/// Parses the program arguments and returns the program options.
///
/// # Arguments
/// * `app_name` - The name of the application.
/// * `version` - The version of the application.
/// * `about` - The description of the application.
pub fn parse_args_and_init_logging(
    app_name: &'static str,
    version: &'static str,
    about: &'static str,
) -> Result<Options> {
    // parse program arguments
    let matches = Command::new(app_name)
        .version(version)
        .about(about)
        .arg(
            arg!(
                -c --config <FILE> "Path to the configuration file."
            )
            .required(true)
            .value_parser(value_parser!(PathBuf)),
        )
        .get_matches();

    let config_path = matches.get_one::<PathBuf>("config").unwrap().clone();

    // load the configuration file, initialize logging and print the configuration
    let program_config = ProgramConfig::try_from(ProgramOptions { config_path })?;
    initialize_logging(LevelFilter::from(program_config.log));
    program_config.print_to_log();

    Ok(Options {
        address: program_config.address,
        postgres: program_config.postgres,
    })
}

/// Runs the program.
async fn run_program() -> Result<()> {
    // read the application name, version and description from the Cargo.toml file
    let (app_name, version, about) = (
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_DESCRIPTION"),
    );

    let _options = parse_args_and_init_logging(app_name, version, about)?;

    info!("Product DB Version: {}", env!("CARGO_PKG_VERSION"));

    todo!("Implement the program logic here");
}

#[actix_web::main]
async fn main() {
    match run_program().await {
        Ok(()) => {
            info!("SUCCESS");
        }
        Err(err) => {
            error!("Error: {}", err);
            eprintln!("{}", err);
            error!("FAILED");

            std::process::exit(-1);
        }
    }
}
