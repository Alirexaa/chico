use http::StatusCode;
use serial_test::serial;
use std::path::Path;

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
            .spawn()
            .expect("Failed to start server");

        ServerFixture { process }
    }

    pub fn stop_app(&mut self) {
        _ = &self.process.kill();
        _ = &self.process.wait();
    }
}

#[tokio::test]
#[serial]
async fn test_respond_handler_ok_with_body_response() {
    let config_file_path =
        Path::new("resources/test_cases/respond-handler/ok_with_body_response.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);

    let response = reqwest::get("http://localhost:3000/").await.unwrap();
    assert_eq!(&response.status(), &StatusCode::OK);
    assert_eq!(&response.text().await.unwrap(), "<h1>Example</h1>");

    app.stop_app();
}

#[tokio::test]
#[serial]
async fn test_respond_handler_403_status_code() {
    let config_file_path = Path::new("resources/test_cases/respond-handler/403_status_code.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);

    let response = reqwest::get("http://localhost:3000/secret/data")
        .await
        .unwrap();
    assert_eq!(&response.status(), &StatusCode::FORBIDDEN);
    assert_eq!(&response.text().await.unwrap(), "Access denied");

    app.stop_app();
}

#[tokio::test]
#[serial]
async fn test_respond_handler_only_body_response() {
    let config_file_path = Path::new("resources/test_cases/respond-handler/only_body_response.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);

    let response = reqwest::get("http://localhost:3000/").await.unwrap();
    assert_eq!(&response.status(), &StatusCode::OK);
    assert_eq!(&response.text().await.unwrap(), "<h1>Example</h1>");

    app.stop_app();
}

#[tokio::test]
#[serial]
async fn test_respond_handler_simple_ok_response() {
    let config_file_path = Path::new("resources/test_cases/respond-handler/simple_ok_response.chf");
    assert!(config_file_path.exists());

    let mut app = ServerFixture::run_app(config_file_path);

    let response = reqwest::get("http://localhost:3000/health").await.unwrap();
    assert_eq!(&response.status(), &StatusCode::OK);
    assert_eq!(&response.text().await.unwrap(), "");

    app.stop_app();
}
