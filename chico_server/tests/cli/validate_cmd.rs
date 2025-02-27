use std::io::Write;

use predicates::prelude::*;
use tempfile::NamedTempFile;

#[test]
fn test_validate_command_without_config_arg_should_return_error() {
    let mut cmd = assert_cmd::Command::cargo_bin("chico").unwrap();
    cmd.arg("validate")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the following required arguments were not provided:\n  --config <CONFIG>",
        ));
}

#[test]
fn test_validate_command_should_return_error_for_invalid_config() {
    // this content have duplicate host, so it's invalid
    let content = r#"
    localhost {
        route / {
            file index.html
        }
    }
    localhost {
        route / {
            file index.html
        }
    }
    "#;

    let mut temp_file = NamedTempFile::new().unwrap();
    let _ = temp_file.write_all(content.as_bytes());
    let file_path = temp_file.path().to_str().unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("chico").unwrap();
    cmd.arg("validate")
        .arg("--config")
        .arg(file_path)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "Failed to parse config file. reason: duplicate domain found: localhost",
        ));
}

#[test]
fn test_validate_command_should_return_success_for_valid_config() {
    let content = r#"
    localhost {
        route / {
            file index.html
        }
    }
    example.com {
        route / {
            file index.html
        }
    }
    "#;

    let mut temp_file = NamedTempFile::new().unwrap();
    let _ = temp_file.write_all(content.as_bytes());
    let file_path = temp_file.path().to_str().unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("chico").unwrap();
    cmd.arg("validate")
        .arg("--config")
        .arg(file_path)
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains(
            "✅✅✅ Specified config is valid.",
        ));
}
