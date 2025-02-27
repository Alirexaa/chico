use clap::{command, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "chico")]
pub(crate) struct CLI {
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
    Run,
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use rstest::rstest;

    use super::{Commands, CLI};

    #[rstest]
    #[case("-c")]
    #[case("--config")]
    fn test_validate_command_parsing(#[case] arg: &str) {
        let args = vec!["chico", "validate", arg, "/path/to/file"];
        let cli = CLI::try_parse_from(args).unwrap();
        // Match the parsed command

        match cli.command {
            Commands::Validate { config } => assert_eq!(config, "/path/to/file"),
            _ => panic!("Expected 'Validate' command"),
        }
    }
}
