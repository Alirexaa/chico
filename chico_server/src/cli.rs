use clap::{command, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "chico")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Validate the config file content
    Validate {
        #[arg(short, long)]
        config: String,
    },
    /// Run the server
    /// This command will block executing shell
    Run {
        #[arg(short, long)]
        config: String,
        #[arg(long, hide = true)]
        daemon_mode: bool,
    },
    /// Start the server as a background daemon
    Start {
        #[arg(short, long)]
        config: String,
    },
    /// Stop the background server daemon
    Stop,
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use rstest::rstest;

    use super::{Cli, Commands};

    #[rstest]
    #[case("-c")]
    #[case("--config")]
    fn test_validate_command_parsing(#[case] arg: &str) {
        let args = vec!["chico", "validate", arg, "/path/to/file"];
        let cli = Cli::try_parse_from(args).unwrap();
        // Match the parsed command

        match cli.command {
            Commands::Validate { config } => assert_eq!(config, "/path/to/file"),
            _ => panic!("Expected 'Validate' command"),
        }
    }

    #[rstest]
    #[case("-c")]
    #[case("--config")]
    fn test_run_command_parsing(#[case] arg: &str) {
        let args = vec!["chico", "run", arg, "/path/to/file"];
        let cli = Cli::try_parse_from(args).unwrap();
        // Match the parsed command

        match cli.command {
            Commands::Run { config, daemon_mode } => {
                assert_eq!(config, "/path/to/file");
                assert!(!daemon_mode);
            },
            _ => panic!("Expected 'Run' command"),
        }
    }

    #[rstest]
    #[case("-c")]
    #[case("--config")]
    fn test_start_command_parsing(#[case] arg: &str) {
        let args = vec!["chico", "start", arg, "/path/to/file"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Start { config } => assert_eq!(config, "/path/to/file"),
            _ => panic!("Expected 'Start' command"),
        }
    }

    #[test]
    fn test_stop_command_parsing() {
        let args = vec!["chico", "stop"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Stop => {}, // Success
            _ => panic!("Expected 'Stop' command"),
        }
    }
}
