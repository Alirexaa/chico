use assert_cmd::Command;
use claims::assert_ok;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

const DAEMON_CONFIG: &str = r#"localhost:18081 {
    route / {
        respond "Hello from daemon test"
    }
}"#;

#[test]
#[serial]
fn test_daemon_start_stop_cycle() {
    // Create a temporary config file
    let config_file = NamedTempFile::new().unwrap();
    fs::write(&config_file, DAEMON_CONFIG).unwrap();
    let config_path = config_file.path().to_str().unwrap();

    // Test starting daemon
    let mut start_cmd = Command::cargo_bin("chico").unwrap();
    start_cmd
        .arg("start")
        .arg("--config")
        .arg(config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Server started as daemon"));

    // Give the daemon a moment to start
    thread::sleep(Duration::from_millis(2000));

    // Test that the server is responding
    let response = reqwest::blocking::get("http://localhost:18081/");
    assert_ok!(response);

    // Test stopping daemon
    let mut stop_cmd = Command::cargo_bin("chico").unwrap();
    stop_cmd
        .arg("stop")
        .assert()
        .success()
        .stdout(predicate::str::contains("Daemon stopped"));

    // Give the daemon a moment to stop
    thread::sleep(Duration::from_millis(1000));

    // Test that the server is no longer responding
    let response = reqwest::blocking::Client::new()
        .get("http://localhost:18081/")
        .timeout(Duration::from_millis(1000))
        .send();
    assert!(response.is_err() || !response.unwrap().status().is_success());
}

#[test]
#[serial]
fn test_start_daemon_already_running() {
    // Create a temporary config file
    let config_file = NamedTempFile::new().unwrap();
    fs::write(&config_file, DAEMON_CONFIG).unwrap();
    let config_path = config_file.path().to_str().unwrap();

    // Start daemon first time
    let mut start_cmd1 = Command::cargo_bin("chico").unwrap();
    start_cmd1
        .arg("start")
        .arg("--config")
        .arg(config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Server started as daemon"));

    // Give the daemon a moment to start
    thread::sleep(Duration::from_millis(1000));

    // Try to start daemon again (should fail)
    let mut start_cmd2 = Command::cargo_bin("chico").unwrap();
    start_cmd2
        .arg("start")
        .arg("--config")
        .arg(config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Daemon already running"));

    // Clean up - stop the daemon
    let mut stop_cmd = Command::cargo_bin("chico").unwrap();
    stop_cmd.arg("stop").assert().success();
    
    thread::sleep(Duration::from_millis(500));
}

#[test]
#[serial]
fn test_stop_daemon_not_running() {
    // Ensure no daemon is running by trying to stop first
    let mut cleanup_cmd = Command::cargo_bin("chico").unwrap();
    let _ = cleanup_cmd.arg("stop").output();

    // Try to stop daemon when none is running
    let mut stop_cmd = Command::cargo_bin("chico").unwrap();
    stop_cmd
        .arg("stop")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No daemon PID file found"));
}

#[test]
#[serial]
fn test_start_daemon_invalid_config() {
    // Create a temporary config file with invalid content
    let config_file = NamedTempFile::new().unwrap();
    fs::write(&config_file, "invalid config content").unwrap();
    let config_path = config_file.path().to_str().unwrap();

    // Try to start daemon with invalid config
    let mut start_cmd = Command::cargo_bin("chico").unwrap();
    start_cmd
        .arg("start")
        .arg("--config")
        .arg(config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Configuration validation failed"));
}