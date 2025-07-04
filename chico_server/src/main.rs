#![cfg_attr(feature = "strict", deny(warnings))]
use clap::Parser;
use config::validate_config_file;
use server::run_server;
use std::{process::ExitCode, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    select,
};
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
            let server = async {
                run_server(conf).await;
            };

            let notify = Arc::new(tokio::sync::Notify::new());
            let notify_clone = notify.clone();
            tokio::spawn(async move {
                let stdin = tokio::io::stdin();
                let mut reader = BufReader::new(stdin).lines();

                while let Ok(Some(line)) = reader.next_line().await {
                    println!("hhhhhhhhhh.");

                    if line.trim() == "shutdown" {
                        println!("Shutdown command received from stdin.");
                        notify_clone.notify_waiters();
                        break;
                    }
                }
            });
            println!("sssssssss");

            let shutdown = async { notify.notified().await };
            println!("ttttttt");

            select! {
                _ = server => {}
                _ = shutdown => {}
            }
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
