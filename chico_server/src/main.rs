#![cfg_attr(feature = "strict", deny(warnings))]
use clap::Parser;
use config::validate_config_file;
use server::run_server;
use std::process::ExitCode;
mod cli;
mod config;
mod daemon;
mod handlers;
mod load_balance;
mod server;
#[cfg(test)]
mod test_utils;
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

            // listen to shutdown from stdio only in tests https://github.com/Alirexaa/chico/issues/99
            #[cfg(feature = "stdin_shutdown")]
            {
                use std::sync::Arc;
                use tokio::select;

                let notify = Arc::new(tokio::sync::Notify::new());
                let notify_clone = notify.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncBufReadExt, BufReader};

                    let stdin = tokio::io::stdin();
                    let mut reader = BufReader::new(stdin).lines();

                    while let Ok(Some(line)) = reader.next_line().await {
                        if line.trim() == "shutdown" {
                            println!("Shutdown command received from stdin.");
                            notify_clone.notify_waiters();
                            break;
                        }
                    }
                });

                let shutdown = async { notify.notified().await };

                select! {
                    _ = server => {}
                    _ = shutdown => {}
                }
            }
            #[cfg(not(feature = "stdin_shutdown"))]
            server.await;

            return ExitCode::SUCCESS;
        }
        cli::Commands::Start { config } => {
            let result = validate_config_file(config.as_str()).await;

            if let Err(e) = result {
                eprintln!("Configuration validation failed: {}", e);
                return ExitCode::FAILURE;
            };

            match daemon::start_daemon(&config) {
                Ok(_) => {
                    println!("✅ Server started as daemon");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Failed to start daemon: {}", e);
                    ExitCode::FAILURE
                }
            }
        }
        cli::Commands::Stop => {
            match daemon::stop_daemon() {
                Ok(_) => {
                    println!("✅ Daemon stopped");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Failed to stop daemon: {}", e);
                    ExitCode::FAILURE
                }
            }
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
