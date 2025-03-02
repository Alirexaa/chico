#![cfg_attr(feature = "strict", deny(warnings))]
use clap::Parser;
use config::validate_config_file;
use server::run_server;
use std::process::exit;
mod cli;
mod config;
mod handlers;
mod server;
mod virtual_host;
#[tokio::main]
async fn main() {
    let cli = cli::CLI::parse();
    match cli.command {
        cli::Commands::Run { config } => {
            let conf = validate_config_file(config.as_str())
                .await
                .unwrap_or_else(|err| {
                    eprintln!("{}", err);
                    exit(1);
                });
            run_server(conf).await
        }
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
