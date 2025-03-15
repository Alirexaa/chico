use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::Stdio,
    sync::{mpsc, Arc, Mutex},
    thread,
};

pub(crate) struct ServerFixture {
    process: std::process::Child,
    executing_dir: String,
    exe_path: String,
    log_receiver: Arc<Mutex<mpsc::Receiver<String>>>, // Store logs for `wait_for_text`
}

impl ServerFixture {
    pub fn run_app<T: AsRef<std::ffi::OsStr>>(config_path: T) -> ServerFixture {
        use assert_cmd::cargo::CommandCargoExt;

        let mut binding = std::process::Command::cargo_bin("chico").expect("Failed to find binary");
        let command = binding
            .arg("run")
            .arg("--config")
            .arg(config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut process = command.spawn().expect("Failed to start server");

        let stdout = process.stdout.take().expect("Failed to capture stdout");
        let stderr = process.stderr.take().expect("Failed to capture stderr");

        // Create channel for log forwarding
        let (log_sender, log_receiver) = mpsc::channel();
        let log_receiver = Arc::new(Mutex::new(log_receiver));

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
        }
    }

    fn log_output<T: std::io::Read + Send + 'static>(
        stream: T,
        label: &'static str,
        sender: mpsc::Sender<String>,
    ) {
        let reader = BufReader::new(stream);
        thread::spawn(move || {
            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("[{}] {}", label, line); // Print logs
                    let _ = sender.send(line); // Send log to channel
                }
            }
        });
    }

    pub fn stop_app(&mut self) {
        // Attempt to kill the process gracefully
        if let Err(e) = self.process.kill() {
            eprintln!("Failed to kill server process: {}", e);
        }

        // Wait for the process to exit, regardless of whether kill succeeded
        if let Err(e) = self.process.wait() {
            eprintln!("Failed to wait for server process: {}", e);
        }
    }

    pub fn wait_for_text(&mut self, text: &str) {
        let log_receiver = self.log_receiver.lock().unwrap();

        loop {
            match log_receiver.recv() {
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

    use http::StatusCode;

    use crate::ServerFixture;

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

        // For this test we don't wait for start
        // Reason following error occurred when unwrap the response
        // thread 'tokio-runtime-worker' panicked at std\src\io\stdio.rs:1123:9:
        //failed printing to stdout: The pipe is being closed. (os error 232)
        // app.wait_for_start();

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
        assert_eq!(&response.status(), &StatusCode::OK);
        assert_eq!(&response.text().await.unwrap(), content);
    }

    #[tokio::test]
    // #[ignore = "reason"]
    async fn test_file_handler_return_404() {
        let config_file_path =
            Path::new("resources/test_cases/file-handler/file_not_exist_return_404.chf");
        assert!(config_file_path.exists());

        let mut app = ServerFixture::run_app(config_file_path);

        // For this test we don't wait for start
        // Reason following error occurred when unwrap the response
        // thread 'tokio-runtime-worker' panicked at std\src\io\stdio.rs:1123:9:
        //failed printing to stdout: The pipe is being closed. (os error 232)
        //thread 'serial_integration::test_file_handler_return_404' panicked at chico_server\tests\server.rs:282:33:
        //called `Result::unwrap()` on an `Err` value: reqwest::Error { kind: Request, url: "http://localhost:3000/not-exist", source: hyper_util::client::legacy::Error(SendRequest, hyper::Error(IncompleteMessage)) }

        // app.wait_for_start();

        let response = reqwest::get("http://localhost:3000/not-exist").await;
        app.stop_app();

        let response = response.unwrap();
        assert_eq!(&response.status(), &StatusCode::NOT_FOUND);
        assert_eq!(&response.text().await.unwrap(), "");
    }
}
