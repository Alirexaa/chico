use std::{
    io::{BufRead, BufReader},
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
        self.process.kill().unwrap();
        self.process.wait().unwrap();
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

/// We use #[serial_test::serial] to run tests (with cargo test) in this module serially. Running these tests concurrency case failure.
/// We use serial_integration name to run tests (with nextest) in this module serially. We configured nextest to run these these serially. See .config/nextest.toml.
#[serial_test::serial]
mod serial_integration {
    use std::{fs::File, io::Write, path::Path};

    use http::StatusCode;

    use crate::ServerFixture;

    #[tokio::test]
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
    async fn test_respond_handler_403_status_code() {
        let config_file_path =
            Path::new("resources/test_cases/respond-handler/403_status_code.chf");
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
    async fn test_respond_handler_only_body_response() {
        let config_file_path =
            Path::new("resources/test_cases/respond-handler/only_body_response.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000/").await.unwrap();
        app.stop_app();

        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(&response.text().await.unwrap(), "<h1>Example</h1>");
    }

    #[tokio::test]
    async fn test_respond_handler_simple_ok_response() {
        let config_file_path =
            Path::new("resources/test_cases/respond-handler/simple_ok_response.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000/health").await.unwrap();
        app.stop_app();

        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(&response.text().await.unwrap(), "");
    }

    #[tokio::test]
    async fn test_redirect_handler_specified_status() {
        let config_file_path =
            Path::new("resources/test_cases/redirect-handler/specified_status.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000/old-path")
            .await
            .unwrap();
        app.stop_app();

        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(
            &response.text().await.unwrap(),
            "<h1>Redirected from old-path</h1>"
        );
    }

    #[tokio::test]
    async fn test_redirect_handler_not_specified_status() {
        let config_file_path =
            Path::new("resources/test_cases/redirect-handler/not_specified_status.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000/old-path")
            .await
            .unwrap();
        app.stop_app();

        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(
            &response.text().await.unwrap(),
            "<h1>Redirected from old-path</h1>"
        );
    }

    #[tokio::test]
    async fn test_respond_handler_return_404_for_unknown_route() {
        let config_file_path =
            Path::new("resources/test_cases/respond-handler/simple_ok_response.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000/blog").await.unwrap();
        app.stop_app();

        let body = r"<!DOCTYPE html>  
<html>  
<head>  
    <title>404 Not Found</title>  
</head>  
<body>  
    <h1>404 Not Found</h1>  
</body>  
</html>";

        assert_eq!(&response.status(), &StatusCode::NOT_FOUND);
        assert_eq!(&response.text().await.unwrap(), body);
    }

    #[tokio::test]
    async fn test_respond_handler_return_404_for_unknown_host() {
        let config_file_path =
            Path::new("resources/test_cases/respond-handler/simple_ok_response.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://127.0.0.1:3000").await.unwrap();
        app.stop_app();
        let body = r"<!DOCTYPE html>  
<html>  
<head>  
    <title>404 Not Found</title>  
</head>  
<body>  
    <h1>404 Not Found</h1>  
</body>  
</html>";
        assert_eq!(&response.status(), &StatusCode::NOT_FOUND);
        assert_eq!(&response.text().await.unwrap(), body);
    }

    #[tokio::test]
    async fn test_respond_handler_return_ok() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let exe_path = std::env::current_exe().unwrap();
        let cd = exe_path.parent().unwrap();
        let file_path = cd.join("index.html");

        let content = r"<!DOCTYPE html>  
        <html>  
        <head>  
            <title>Hello World</title>  
        </head>  
        <body>  
            <h1>Hello World</h1>  
        </body>  
        </html>";

        let mut file = File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000").await;
        app.stop_app();
        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(&response.text().await.unwrap(), content);
    }

    #[tokio::test]
    async fn test_file_handler_return_404() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_not_exist_return_404.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000/not-exist").await;
        app.stop_app();

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::NOT_FOUND);
        assert_eq!(&response.text().await.unwrap(), "");
    }
}
