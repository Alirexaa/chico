use std::{env, io::ErrorKind, path::PathBuf};

use futures_util::TryStreamExt;
use http::{Response, StatusCode};
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::handlers::respond::RespondHandler;

use super::{BoxBody, RequestHandler};

static MIME_DICT: std::sync::LazyLock<mimee::MimeDict> =
    std::sync::LazyLock::new(|| mimee::MimeDict::new());

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
    async fn handle(
        &self,
        _request: hyper::Request<impl hyper::body::Body>,
    ) -> http::Response<BoxBody> {
        let mut path = PathBuf::from(&self.path);

        if !path.is_absolute() {
            let exe_path = env::current_exe().unwrap();
            let cd = exe_path.parent().unwrap();
            path = cd.join(path);
        };

        let path_exist = tokio::fs::try_exists(&path).await;

        if let Ok(true) = path_exist {
            let metadata = tokio::fs::metadata(&path).await;
            if metadata.is_err() {
                let err_kind = metadata.as_ref().err().unwrap().kind();
                return handle_file_error(_request, err_kind).await;
            }
            let metadata = metadata.unwrap();
            if metadata.is_dir() {
                if !self.is_dir {
                    return handle_file_error(_request, ErrorKind::IsADirectory).await;
                }
            }
        } else {
            return handle_file_error(_request, ErrorKind::NotFound).await;
        }

        if self.is_dir {
            let ending = extract_ending_from_req_path(&_request.uri().path(), &self.route);
            if ending.is_none() {
                return handle_file_error(_request, ErrorKind::NotFound).await;
            }
            path = path.join(ending.unwrap());
        };

        let file = File::open(&path).await;

        if file.is_err() {
            let err_kind = file.as_ref().err().unwrap().kind();
            return handle_file_error(_request, err_kind).await;
        }
        let file: File = file.unwrap();
        process_file(path.to_str().unwrap(), file)
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

fn process_file(
    file_name: &str,
    file: File,
) -> Response<http_body_util::combinators::BoxBody<bytes::Bytes, std::io::Error>> {
    let mut builder = Response::builder().status(StatusCode::OK);

    let content_type = MIME_DICT.get_content_type(file_name);
    if content_type.is_some() {
        builder = builder.header(http::header::CONTENT_TYPE, content_type.unwrap());
    }

    let reader_stream = ReaderStream::new(file);

    // Convert to http_body_util::BoxBody
    let stream_body = StreamBody::new(reader_stream.map_ok(Frame::data));
    let boxed_body = stream_body.boxed();

    let response = builder.body(boxed_body).unwrap();
    response
}

async fn handle_file_error(
    _request: http::Request<impl hyper::body::Body>,
    error: ErrorKind,
) -> Response<BoxBody> {
    let handler = match error {
        ErrorKind::NotFound => RespondHandler::not_found(),
        ErrorKind::PermissionDenied => RespondHandler::forbidden(),
        ErrorKind::IsADirectory => RespondHandler::forbidden(),
        _ => RespondHandler::internal_server_error(),
    };
    return handler.handle(_request).await;
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

    use crate::{
        handlers::{file::FileHandler, respond::RespondHandler, RequestHandler},
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
        let mut file = File::create(&file_path).unwrap();
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
        let result = extract_ending_from_req_path(req_path, &route);
        assert_eq!(ending.to_string(), result.unwrap());
    }
}
