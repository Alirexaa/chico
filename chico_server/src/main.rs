#![cfg_attr(feature = "strict", deny(warnings))]
use clap::Parser;
use config::validate_config_file;
use server::run_server;
use std::process::ExitCode;
mod cli;
mod config;
mod handlers;
mod server;
#[cfg(test)]
mod test_utils;
mod uri;
mod virtual_host;
#[tokio::main]
async fn main() -> ExitCode {
    crates_tracing::init("chico.log".to_string(), "chico".to_string());

    let cli = cli::Cli::parse();
    match cli.command {
        cli::Commands::Run { config } => {
            let result = validate_config_file(config.as_str()).await;

            let Ok(conf) = result else {
                eprintln!("{}", result.err().unwrap());
                return ExitCode::FAILURE;
            };
            run_server(conf).await;
            return ExitCode::SUCCESS;
        }
        cli::Commands::Validate { config } => {
            let result = validate_config_file(config.as_str()).await;

            if let Err(e) = result {
                eprintln!("{}", e);
                return ExitCode::FAILURE;
            };
            println!("✅✅✅ Specified config is valid.");
            return ExitCode::SUCCESS;
        }
    }
}
