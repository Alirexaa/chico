#![cfg_attr(feature = "strict", deny(warnings))]

use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{ChildStdin, Stdio},
    sync::mpsc,
    thread,
};

pub(crate) struct ServerFixture {
    process: std::process::Child,
    executing_dir: String,
    exe_path: String,
    log_receiver: mpsc::Receiver<String>, // Store logs for `wait_for_text`
    stdin: ChildStdin,
    has_shutdown: bool,
}

impl ServerFixture {
    pub fn run_app<T: AsRef<std::ffi::OsStr>>(config_path: T) -> ServerFixture {
        use assert_cmd::cargo::CommandCargoExt;

        let mut binding = std::process::Command::cargo_bin("chico").expect("Failed to find binary");
        let command = binding
            .arg("run")
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut process = command.spawn().expect("Failed to start server");

        let stdout = process.stdout.take().expect("Failed to capture stdout");
        let stderr = process.stderr.take().expect("Failed to capture stderr");
        let stdin = process.stdin.take().expect("Failed to capture stdin");

        // Create channel for log forwarding
        let (log_sender, log_receiver) = mpsc::channel();

        // Spawn threads to log stdout and stderr
        ServerFixture::log_output(stdout, "STDOUT", log_sender.clone());
        ServerFixture::log_output(stderr, "STDERR", log_sender.clone());

        let program_path = Path::new(command.get_program());
        let exe_path = program_path.to_str().unwrap().to_string();
        let executing_dir = program_path.parent().unwrap().to_str().unwrap().to_string();
        ServerFixture {
            process,
            executing_dir,
            exe_path,
            log_receiver,
            stdin,
            has_shutdown: false,
        }
    }

    fn log_output<T: std::io::Read + Send + 'static>(
        stream: T,
        label: &'static str,
        sender: mpsc::Sender<String>,
    ) {
        let reader = BufReader::new(stream);
        thread::spawn(move || {
            for line in reader.lines().map_while(Result::ok) {
                println!("[{}] {}", label, line); // Print logs
                let _ = sender.send(line); // Send log to channel
            }
        });
    }
    pub fn stop_app(&mut self) {
        if self.has_shutdown {
            println!("Server already shut down, skipping stop_app.");
            return;
        }

        println!("stoppppppp");

        match self.process.try_wait() {
            Ok(Some(status)) => {
                println!("Process already exited with: {}", status);
                self.has_shutdown = true;
                return;
            }
            Ok(None) => {
                // still running
            }
            Err(e) => {
                eprintln!("Failed to check process status: {}", e);
                return;
            }
        }

        match self.stdin.write_all(b"shutdown\n") {
            Ok(_) => {
                println!("shutdown command sent");
            }
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                eprintln!("Broken pipe: stdin already closed");
            }
            Err(e) => {
                eprintln!("Failed to write to stdin: {}", e);
            }
        }

        if let Err(e) = self.stdin.flush() {
            eprintln!("Failed to flush stdin: {}", e);
        }

        if let Err(e) = self.process.wait() {
            eprintln!("Failed to wait for server process: {}", e);
        }

        self.has_shutdown = true; // Mark as shut down
    }

    pub fn wait_for_text(&mut self, text: &str) {
        loop {
            match self.log_receiver.recv() {
                Ok(line) => {
                    if line.contains(text) {
                        return;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Could not wait for text. The log_receiver channel is closed. error: {}",
                        e
                    );
                    return;
                }
            }
        }
    }

    pub fn wait_for_start(&mut self) {
        self.wait_for_text("Start listening to incoming requests on");
    }

    #[allow(dead_code)]
    pub fn get_executing_dir(&self) -> &String {
        &self.executing_dir
    }
    #[allow(dead_code)]
    pub fn get_current_exe(&self) -> &String {
        &self.exe_path
    }
}

impl Drop for ServerFixture {
    fn drop(&mut self) {
        println!("Dropping ServerFixture, stopping app...");
        self.stop_app();
        println!("ServerFixture dropped.");
    }
}

/// We use #[serial_test::serial] to run tests (with cargo test) in this module serially. Running these tests concurrency case failure.
/// We use serial_integration name to run tests (with nextest) in this module serially. We configured nextest to run these these serially. See .config/nextest.toml.
#[serial_test::serial]
mod serial_integration {
    use std::{fs::File, io::Write, path::Path};

    use crate::ServerFixture;
    use http::StatusCode;
    #[tokio::test]
    async fn test_respond_handler_ok_with_body_response() {
        let config_file_path =
            Path::new("resources/test_cases/respond-handler/ok_with_body_response.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);
        app.wait_for_start();
        let response = reqwest::get("http://localhost:3000/").await;
        app.stop_app();

        let response = response.unwrap();
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
        let response = reqwest::get("http://localhost:3000/secret/data").await;

        app.stop_app();

        let response = response.unwrap();
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
        let response = reqwest::get("http://localhost:3000/").await;
        app.stop_app();

        let response = response.unwrap();
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
        let response = reqwest::get("http://localhost:3000/health").await;
        app.stop_app();

        let response = response.unwrap();
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
        let response = reqwest::get("http://localhost:3000/old-path").await;
        app.stop_app();

        let response = response.unwrap();
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
        let response = reqwest::get("http://localhost:3000/old-path").await;
        app.stop_app();

        let response = response.unwrap();
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
        let response = reqwest::get("http://localhost:3000/blog").await;
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

        let response = response.unwrap();
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
        let response = reqwest::get("http://127.0.0.1:3000").await;
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

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::NOT_FOUND);
        assert_eq!(&response.text().await.unwrap(), body);
    }

    #[tokio::test]
    async fn test_file_handler_return_ok() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let file_path = Path::new(app.get_executing_dir()).join("index.html");

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

        app.wait_for_start();

        let response = reqwest::get("http://localhost:3000").await;

        // Cleanup resources before assertion
        app.stop_app();
        _ = std::fs::remove_file(file_path);

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/html"
        );
        assert_eq!(&response.text().await.unwrap(), content);
    }

    #[tokio::test]
    async fn test_file_handler_return_ok_dynamic_route() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let dir = Path::new(app.get_executing_dir()).join("srv/downloads");
        let file_path = &dir.join("hello.txt");

        let content = r"Hello World!!!";

        std::fs::create_dir_all(dir).expect("Expected to create directories");
        let mut file = File::create(file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();

        app.wait_for_start();

        let response = reqwest::get("http://localhost:3000/downloads/hello.txt").await;

        // Cleanup resources before assertion
        app.stop_app();
        _ = std::fs::remove_file(file_path);

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/plain"
        );
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

    #[tokio::test]
    async fn test_file_handler_head_request() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let file_path = Path::new(app.get_executing_dir()).join("index.html");

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

        app.wait_for_start();

        let response = reqwest::Client::new()
            .head("http://localhost:3000")
            .send()
            .await;

        // Cleanup resources before assertion
        app.stop_app();
        _ = std::fs::remove_file(file_path);

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/html"
        );
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .unwrap()
                .to_str()
                .unwrap(),
            content.len().to_string()
        );
        assert_eq!(
            response
                .headers()
                .get(http::header::ACCEPT_RANGES)
                .unwrap()
                .to_str()
                .unwrap(),
            "bytes"
        );
    }

    #[tokio::test]
    async fn test_file_handler_valid_range_request() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let file_path = Path::new(app.get_executing_dir()).join("test.txt");

        let content = b"Hello, this is a test file content!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();

        app.wait_for_start();

        let response = reqwest::Client::new()
            .get("http://localhost:3000/test.txt")
            .header(http::header::RANGE, "bytes=0-4")
            .send()
            .await;

        // Cleanup resources before assertion
        app.stop_app();
        _ = std::fs::remove_file(file_path);

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_RANGE)
                .unwrap()
                .to_str()
                .unwrap(),
            "bytes 0-4/35"
        );
        assert_eq!(response.text().await.unwrap(), "Hello");
    }

    #[tokio::test]
    async fn test_file_handler_invalid_range_request() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let file_path = Path::new(app.get_executing_dir()).join("test.txt");

        let content = b"Hello, this is a test file content!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();

        app.wait_for_start();

        let response = reqwest::Client::new()
            .get("http://localhost:3000/test.txt")
            .header(http::header::RANGE, "bytes=50-60")
            .send()
            .await;

        // Cleanup resources before assertion
        app.stop_app();
        _ = std::fs::remove_file(file_path);

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_RANGE)
                .unwrap()
                .to_str()
                .unwrap(),
            "bytes */35"
        );
        assert_eq!(response.text().await.unwrap(), "");
    }

    #[tokio::test]
    async fn test_file_handler_disallow_methods() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let file_path = Path::new(app.get_executing_dir()).join("test.txt");

        let content = b"Hello, this is a test file content!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();

        app.wait_for_start();

        let disallowed_methods = vec![
            http::Method::POST,
            http::Method::PUT,
            http::Method::DELETE,
            http::Method::PATCH,
            http::Method::OPTIONS,
        ];

        for method in disallowed_methods {
            let client = reqwest::Client::new();
            let response = client
                .request(method.clone(), "http://localhost:3000/test.txt")
                .send()
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(
                response
                    .headers()
                    .get(http::header::ALLOW)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "GET, HEAD"
            );
        }

        // Cleanup resources
        app.stop_app();
        _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn test_file_handler_allow_methods() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_exist_return_ok.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let file_path = Path::new(app.get_executing_dir()).join("test.txt");

        let content = b"Hello, this is a test file content!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();

        app.wait_for_start();

        let allowed_methods = vec![http::Method::GET, http::Method::HEAD];

        for method in allowed_methods {
            let client = reqwest::Client::new();
            let response = client
                .request(method.clone(), "http://localhost:3000/test.txt")
                .send()
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        // Cleanup resources
        app.stop_app();
        _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn test_reverse_proxy_handler_proxied_request() {
        let config_file_path =
            Path::new("resources/test_cases/reverse-proxy-handler/reverse-proxy-sample-1.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        let file_path = Path::new(app.get_executing_dir()).join("test.txt");

        let content = b"Hello, this is a test file content!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();

        app.wait_for_start();

        let response = reqwest::Client::new()
            .get("http://127.0.0.1:4000")
            .send()
            .await;

        // Cleanup resources before assertion
        app.stop_app();
        _ = std::fs::remove_file(file_path);

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello");
    }
}
