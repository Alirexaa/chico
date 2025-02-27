#![cfg_attr(feature = "strict", deny(warnings))]
use std::process::exit;

use clap::Parser;
use config::validate_config_file;
mod cli;
mod config;
#[tokio::main]
async fn main() {
    let cli = cli::CLI::parse();
    match cli.command {
        cli::Commands::Run => todo!(),
        cli::Commands::Validate { config } => {
            validate_config_file(config.as_str())
                .await
                .unwrap_or_else(|err| {
                    eprintln!("{}", err);
                    exit(1);
                });
            println!("✅✅✅ Specified config is valid.");
            exit(0);
        }
    }
}
