use http::StatusCode;
use serial_test::serial;
use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::Stdio,
    time::Duration,
};

pub(crate) struct ServerFixture {
    process: std::process::Child,
}

impl ServerFixture {
    pub fn run_app<T: AsRef<std::ffi::OsStr>>(config_path: T) -> ServerFixture {
        use assert_cmd::cargo::CommandCargoExt;

        let process = std::process::Command::cargo_bin("chico")
            .expect("Failed to find binary")
            .arg("run")
            .arg("--config")
            .arg(config_path)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start server");

        ServerFixture { process }
    }

    pub fn stop_app(&mut self) {
        let _ = &self.process.kill().unwrap();
        _ = &self.process.wait();
        self.wait_for_port_release();
    }

    fn wait_for_port_release(&self) {
        let max_retries = 10;
        let delay = Duration::from_millis(300);

        for _ in 0..max_retries {
            if std::net::TcpListener::bind("127.0.0.1:3000").is_ok() {
                return; // Port is free, server is fully stopped
            }
            std::thread::sleep(delay);
        }

        panic!("Server did not release port 3000 within expected time");
    }

    pub fn wait_for_text(&mut self, text: &str) {
        let stdout = self
            .process
            .stdout
            .take()
            .expect("Failed to capture stdout");
        let reader = BufReader::new(stdout);

        // Wait for the expected log line before proceeding
        for line in reader.lines() {
            let line = line.expect("Failed to read log line");
            if line.contains(text) {
                break;
            }
        }
    }

    pub fn wait_for_start(&mut self) {
        self.wait_for_text("Start listening requests on");
    }
}

#[tokio::test]
#[serial]
async fn test_respond_handler_ok_with_body_response() {
    let config_file_path =
        Path::new("resources/test_cases/respond-handler/ok_with_body_response.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);
    app.wait_for_start();
    let response = reqwest::get("http://localhost:3000/").await.unwrap();
    app.stop_app();

    assert_eq!(&response.status(), &StatusCode::OK);
    assert_eq!(&response.text().await.unwrap(), "<h1>Example</h1>");
}

#[tokio::test]
#[serial]
async fn test_respond_handler_403_status_code() {
    let config_file_path = Path::new("resources/test_cases/respond-handler/403_status_code.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);
    app.wait_for_start();
    let response = reqwest::get("http://localhost:3000/secret/data")
        .await
        .unwrap();
    app.stop_app();

    assert_eq!(&response.status(), &StatusCode::FORBIDDEN);
    assert_eq!(&response.text().await.unwrap(), "Access denied");
}

#[tokio::test]
#[serial]
async fn test_respond_handler_only_body_response() {
    let config_file_path = Path::new("resources/test_cases/respond-handler/only_body_response.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);
    app.wait_for_start();
    let response = reqwest::get("http://localhost:3000/").await.unwrap();
    app.stop_app();

    assert_eq!(&response.status(), &StatusCode::OK);
    assert_eq!(&response.text().await.unwrap(), "<h1>Example</h1>");
}

#[tokio::test]
#[serial]
async fn test_respond_handler_simple_ok_response() {
    let config_file_path = Path::new("resources/test_cases/respond-handler/simple_ok_response.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);
    app.wait_for_start();
    let response = reqwest::get("http://localhost:3000/health").await.unwrap();
    app.stop_app();

    assert_eq!(&response.status(), &StatusCode::OK);
    assert_eq!(&response.text().await.unwrap(), "");
}
