use std::{
    env,
    fs::Metadata,
    io::{ErrorKind, SeekFrom},
    path::PathBuf,
};

use futures_util::TryStreamExt;
use http::{Method, Response, StatusCode};
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};
use tokio_util::io::ReaderStream;

use crate::handlers::respond::RespondHandler;

use super::{full, BoxBody, RequestHandler};

static MIME_DICT: std::sync::LazyLock<mimee::MimeDict> =
    std::sync::LazyLock::new(mimee::MimeDict::new);

#[derive(PartialEq, Debug)]
pub struct FileHandler {
    pub path: String,
    pub is_dir: bool,
    pub route: String,
}

impl FileHandler {
    pub fn new(path: String, route: String) -> FileHandler {
        FileHandler {
            is_dir: path.ends_with("/"),
            path,
            route,
        }
    }
}

impl RequestHandler for FileHandler {
    async fn handle<B>(&self, request: hyper::Request<B>) -> Response<super::BoxBody>
    where
        B: hyper::body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let req_method = request.method();
        if req_method != http::Method::GET && req_method != http::Method::HEAD {
            return http::response::Builder::new()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header(http::header::ALLOW, "GET, HEAD")
                .body(full(""))
                .unwrap();
        }
        let mut path = PathBuf::from(&self.path);

        if !path.is_absolute() {
            let exe_path = env::current_exe().unwrap();
            let cd = exe_path.parent().unwrap();
            path = cd.join(path);
        };

        let path_exist = tokio::fs::try_exists(&path).await;

        if !path_exist.is_ok_and(|x: bool| x) {
            return handle_file_error(request, ErrorKind::NotFound).await;
        }

        let metadata = tokio::fs::metadata(&path).await;
        if metadata.is_err() {
            let err_kind = metadata.as_ref().err().unwrap().kind();
            return handle_file_error(request, err_kind).await;
        }

        let metadata = &metadata.unwrap();
        if metadata.is_dir() && !self.is_dir {
            return handle_file_error(request, ErrorKind::IsADirectory).await;
        }

        if self.is_dir {
            let ending = extract_ending_from_req_path(request.uri().path(), &self.route);
            if ending.is_none() {
                return handle_file_error(request, ErrorKind::NotFound).await;
            }
            path = path.join(ending.unwrap());
        };

        let file = File::open(&path).await;

        if file.is_err() {
            let err_kind = file.as_ref().err().unwrap().kind();
            return handle_file_error(request, err_kind).await;
        }

        let metadata = tokio::fs::metadata(&path).await;
        if metadata.is_err() {
            let err_kind = metadata.as_ref().err().unwrap().kind();
            return handle_file_error(request, err_kind).await;
        }
        let file: File = file.unwrap();
        let metadata = &metadata.unwrap();
        process_file(request, path.to_str().unwrap(), file, metadata).await
    }
}

fn extract_ending_from_req_path(req_path: &str, route: &str) -> Option<String> {
    let slash_index = route.rfind("/*")?;
    let route_without_asterisk = &route[..=slash_index];
    let route_without_asterisk_length = route_without_asterisk.len();
    let i = req_path.find(route_without_asterisk)?;
    let ending = &req_path[i + route_without_asterisk_length..];
    Some(ending.to_string())
}

async fn process_file<B>(
    request: hyper::Request<B>,
    file_name: &str,
    mut file: File,
    metadata: &Metadata,
) -> Response<BoxBody>
where
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let mut builder = Response::builder();

    let content_type = MIME_DICT.get_content_type(file_name);
    let file_size = metadata.len();

    if content_type.is_some() {
        builder = builder.header(http::header::CONTENT_TYPE, content_type.unwrap());
    }

    if *request.method() == Method::HEAD {
        builder = builder.header(http::header::CONTENT_LENGTH, file_size);
    }
    builder = builder.header(http::header::ACCEPT_RANGES, "bytes");

    let range_header = request.headers().get(http::header::RANGE);
    let mut range = None;
    if range_header.is_some() {
        match range_header.unwrap().to_str() {
            Ok(data) => match parse_range(data, file_size) {
                Some(r) => range = Some(Ok(r)),
                None => range = Some(Err("Invalid range")),
            },
            Err(_) => range = Some(Err("Invalid Header")),
        }
    }

    if range.is_some() {
        let range = range.unwrap();
        if range.is_err() {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(
                    http::header::CONTENT_RANGE,
                    format!("bytes */{}", file_size),
                )
                .body(super::full(""))
                .unwrap();
        }

        let range = range.unwrap();
        let (start, end) = range[0];
        let content_length = end - start + 1;

        if let Err(e) = file.seek(SeekFrom::Start(start)).await {
            return handle_file_error(request, e.kind()).await;
        };
        let stream = ReaderStream::new(file.take(content_length));
        let stream_body = StreamBody::new(stream.map_ok(Frame::data));
        let boxed_body = stream_body.boxed();

        builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(http::header::CONTENT_LENGTH, content_length)
            .header(
                http::header::CONTENT_RANGE,
                format!("bytes {}-{}/{}", start, end, file_size),
            )
            .body(boxed_body)
            .unwrap()
    } else {
        let reader_stream = ReaderStream::new(file);
        let stream_body = StreamBody::new(reader_stream.map_ok(Frame::data));
        let boxed_body = stream_body.boxed();

        builder.status(StatusCode::OK).body(boxed_body).unwrap()
    }
}

async fn handle_file_error<B>(request: hyper::Request<B>, error: ErrorKind) -> Response<BoxBody>
where
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let handler = match error {
        ErrorKind::NotFound => RespondHandler::not_found(),
        ErrorKind::PermissionDenied => RespondHandler::forbidden(),
        ErrorKind::IsADirectory => RespondHandler::forbidden(),
        _ => RespondHandler::internal_server_error(),
    };
    return handler.handle(request).await;
}

/// Helper function to parse Range header
/// Returns None if the range is invalid
#[allow(dead_code)]
fn parse_range(range: &str, file_size: u64) -> Option<Vec<(u64, u64)>> {
    if !range.starts_with("bytes=") {
        return None;
    }

    let range = &range[6..]; // Strip "bytes="
    let mut ranges = Vec::new();

    for part in range.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start, end)) = part.split_once('-') {
            match (start.parse::<u64>().ok(), end.parse::<u64>().ok()) {
                (Some(start), Some(end)) if start <= end && end < file_size => {
                    ranges.push((start, end));
                }
                (Some(start), None) if start < file_size => {
                    ranges.push((start, file_size - 1)); // Start at `start`, go to end
                }
                (None, Some(end)) if end > 0 => {
                    let start = file_size.saturating_sub(end); // Last `end` bytes
                    ranges.push((start, file_size - 1));
                }
                _ => return None, // Invalid range
            }
        }
    }

    if ranges.is_empty() {
        None
    } else {
        Some(ranges)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{ErrorKind, Write},
    };

    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use rstest::rstest;
    use tempfile::NamedTempFile;

    use crate::{
        handlers::{
            file::{parse_range, FileHandler},
            respond::RespondHandler,
            RequestHandler,
        },
        test_utils::MockBody,
    };

    use super::extract_ending_from_req_path;

    #[tokio::test]
    async fn test_file_handler_return_ok_relative_path() {
        // For relative file we try to lookup file in directory or sub-directory of exe location
        // Create file in executing directory
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

        let file_handler = FileHandler::new("index.html".to_string(), "/".to_string());

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = file_handler.handle(request).await;

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

        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(response_body, content);

        // Ignore the result of removing file
        _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn test_file_handler_head_request_set_required_headers() {
        let content = r"<!DOCTYPE html>  
        <html>  
        <head>  
            <title>Hello World</title>  
        </head>  
        <body>  
            <h1>Hello World</h1>  
        </body>  
        </html>";

        let mut temp_file = NamedTempFile::with_suffix(".html").unwrap();
        temp_file
            .write_all(content.as_bytes())
            .expect("Expected to write content");

        let file_path = temp_file.path().to_str().unwrap();

        let file_handler = FileHandler::new(file_path.to_string(), "/".to_string());

        let file_size = temp_file
            .as_file()
            .metadata()
            .expect("Expected to get metadata")
            .len();

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder()
            .method(http::method::Method::HEAD)
            .body(request_body)
            .unwrap();

        let response = file_handler.handle(request).await;

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
            file_size.to_string()
        );
        assert_eq!(
            response
                .headers()
                .get(http::header::ACCEPT_RANGES)
                .unwrap()
                .to_str()
                .unwrap(),
            "bytes".to_string()
        );

        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(response_body, content);
    }

    #[tokio::test]
    async fn test_file_handler_return_ok_relative_path_and_dynamic_route() {
        // For relative file we try to lookup file in directory or sub-directory of exe location
        // Create file in executing directory
        let exe_path = std::env::current_exe().unwrap();
        let cd = exe_path.parent().unwrap();
        let dir = cd.join("srv/dir1/dir2");
        let file_path = &dir.join("hello.txt");

        let content = r"Hello world!!!";
        println!("{:?}", file_path);
        std::fs::create_dir_all(dir).expect("Expected to create directories");
        let mut file = File::create(file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();

        // This is file handler for following config
        // localhost {
        //     route /* {
        //         file srv/
        //     }
        // }

        let file_handler = FileHandler::new("srv/".to_string(), "/*".to_string());

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder()
            .uri("http://localhost/dir1/dir2/hello.txt")
            .body(request_body)
            .unwrap();

        let response = file_handler.handle(request).await;

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

        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(response_body, content);

        // Ignore the result of removing file
        _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn test_file_handler_return_ok_absolute_path() {
        let exe_path = std::env::current_exe().unwrap();
        let cd = exe_path.parent().unwrap();
        let file_path = cd.join("index2.html");

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

        let file_handler =
            FileHandler::new(file_path.to_str().unwrap().to_string(), "/".to_string());
        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = file_handler.handle(request).await;

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
        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(response_body, content);

        // Ignore the result of removing file
        _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn test_file_handler_return_404() {
        let file_handler = FileHandler::new("not-exist-index.html".to_string(), "/".to_string());
        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = file_handler.handle(request).await;

        assert_eq!(&response.status(), &StatusCode::NOT_FOUND);
        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(response_body, "");
    }

    #[tokio::test]
    async fn test_file_handler_return_403() {
        let exe_path = std::env::current_exe().unwrap();
        let cd = exe_path.parent().unwrap();

        // Try to reading content of directory as file case PermissionDenied by OS
        let file_handler = FileHandler::new(cd.to_str().unwrap().to_string(), "/".to_string());
        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = file_handler.handle(request).await;

        assert_eq!(&response.status(), &StatusCode::FORBIDDEN);
        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(response_body, "");
    }

    #[tokio::test]
    #[rstest]
    #[case(ErrorKind::NotFound, RespondHandler::not_found())]
    #[case(ErrorKind::PermissionDenied, RespondHandler::forbidden())]
    #[case(ErrorKind::IsADirectory, RespondHandler::forbidden())]
    async fn test_handle_file_error(#[case] error: ErrorKind, #[case] handler: RespondHandler) {
        use crate::handlers::file::handle_file_error;

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();
        let actual_response = handle_file_error(request.clone(), error).await;
        let expected_response = handler.handle(request).await;
        assert_eq!(expected_response.status(), actual_response.status());
        assert_eq!(
            expected_response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
            actual_response.boxed().collect().await.unwrap().to_bytes()
        )
    }

    #[rstest]
    #[case("/*", "/downloads/dir1/file.txt", "downloads/dir1/file.txt")]
    #[case("/downloads/*", "/downloads/dir1/file.txt", "dir1/file.txt")]
    #[case("/downloads/*", "/downloads/dir1/dir2/file.txt", "dir1/dir2/file.txt")]
    #[case(
        "/downloads/v1/*",
        "/downloads/v1/dir1/dir2/file.txt",
        "dir1/dir2/file.txt"
    )]
    #[case("/downloads/v1/*", "/downloads/v1/فایل.txt", "فایل.txt")]
    #[case(
        "/downloads/v1/*",
        "/downloads/v1/Löwe 老虎 Léopard Gepardi.txt",
        "Löwe 老虎 Léopard Gepardi.txt"
    )]
    fn test_extract_ending_from_req_path(
        #[case] route: &str,
        #[case] req_path: &str,
        #[case] ending: &str,
    ) {
        let result = extract_ending_from_req_path(req_path, route);
        assert_eq!(ending.to_string(), result.unwrap());
    }

    #[test]
    fn test_parse_range_valid_ranges() {
        let file_size = 100;

        // Single valid range
        let range = "bytes=0-49";
        let result = parse_range(range, file_size);
        assert_eq!(result, Some(vec![(0, 49)]));

        // Multiple valid ranges
        let range = "bytes=0-49,50-99";
        let result = parse_range(range, file_size);
        assert_eq!(result, Some(vec![(0, 49), (50, 99)]));

        // Open-ended range (start only)
        let range = "bytes=50-";
        let result = parse_range(range, file_size);
        assert_eq!(result, Some(vec![(50, 99)]));

        // Open-ended range (end only)
        let range = "bytes=-10";
        let result = parse_range(range, file_size);
        assert_eq!(result, Some(vec![(90, 99)]));
    }

    #[test]
    fn test_parse_range_invalid_ranges() {
        let file_size = 100;

        // Invalid range format
        let range = "bytes=abc-def";
        let result = parse_range(range, file_size);
        assert_eq!(result, None);

        // Start greater than end
        let range = "bytes=50-40";
        let result = parse_range(range, file_size);
        assert_eq!(result, None);

        // End exceeds file size
        let range = "bytes=90-110";
        let result = parse_range(range, file_size);
        assert_eq!(result, None);

        // Missing "bytes=" prefix
        let range = "0-49";
        let result = parse_range(range, file_size);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_range_edge_cases() {
        let file_size = 100;

        // Empty range
        let range = "bytes=";
        let result = parse_range(range, file_size);
        assert_eq!(result, None);

        // Range with whitespace
        let range = "bytes= 0-49 , 50-99 ";
        let result = parse_range(range, file_size);
        assert_eq!(result, Some(vec![(0, 49), (50, 99)]));

        // Range covering the entire file
        let range = "bytes=0-99";
        let result = parse_range(range, file_size);
        assert_eq!(result, Some(vec![(0, 99)]));
    }

    #[tokio::test]
    async fn test_file_handler_valid_range() {
        let content = b"Hello, this is a test file content!";
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        temp_file.write_all(content).unwrap();

        let file_path = temp_file.path().to_str().unwrap().to_string();
        let file_handler = FileHandler::new(file_path.clone(), "/".to_string());

        let request = http::Request::builder()
            .header(http::header::RANGE, "bytes=0-4")
            .body(MockBody::new(b""))
            .unwrap();

        let response = file_handler.handle(request).await;

        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_RANGE)
                .unwrap()
                .to_str()
                .unwrap(),
            "bytes 0-4/35"
        );

        let response_body = response.boxed().collect().await.unwrap().to_bytes();
        assert_eq!(*response_body, *b"Hello");
    }

    #[tokio::test]
    async fn test_file_handler_invalid_range() {
        let content = b"Hello, this is a test file content!";
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        temp_file.write_all(content).unwrap();

        let file_path = temp_file.path().to_str().unwrap().to_string();
        let file_handler = FileHandler::new(file_path.clone(), "/".to_string());

        let request = http::Request::builder()
            .header(http::header::RANGE, "bytes=50-60")
            .body(MockBody::new(b""))
            .unwrap();

        let response = file_handler.handle(request).await;

        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_RANGE)
                .unwrap()
                .to_str()
                .unwrap(),
            "bytes */35"
        );

        let response_body = response.boxed().collect().await.unwrap().to_bytes();
        assert_eq!(*response_body, *b"");
    }

    #[tokio::test]
    #[rstest]
    #[case(http::Method::POST)]
    #[case(http::Method::PUT)]
    #[case(http::Method::DELETE)]
    #[case(http::Method::PATCH)]
    #[case(http::Method::OPTIONS)]
    async fn test_file_handler_disallow_methods(#[case] method: http::Method) {
        let file_handler = FileHandler::new("index.html".to_string(), "/".to_string());

        let request_body: MockBody = MockBody::new(b"");
        let request = http::Request::builder()
            .method(method)
            .body(request_body)
            .unwrap();

        let response = file_handler.handle(request).await;

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

    #[tokio::test]
    #[rstest]
    #[case(http::Method::GET)]
    #[case(http::Method::HEAD)]
    async fn test_file_handler_allow_methods(#[case] method: http::Method) {
        let content = b"Hello, this is a test file content!";
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        temp_file.write_all(content).unwrap();

        let file_path = temp_file.path().to_str().unwrap().to_string();
        let file_handler = FileHandler::new(file_path.clone(), "/".to_string());

        let request_body: MockBody = MockBody::new(b"");
        let request = http::Request::builder()
            .method(method)
            .body(request_body)
            .unwrap();

        let response = file_handler.handle(request).await;

        assert_eq!(response.status(), StatusCode::OK);
    }
}
