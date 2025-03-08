use std::{env, io::ErrorKind, path::PathBuf};

use chico_file::types;
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
    pub handler: types::Handler,
}

impl RequestHandler for FileHandler {
    async fn handle(
        &self,
        _request: hyper::Request<impl hyper::body::Body>,
    ) -> http::Response<BoxBody> {
        if let types::Handler::File(file_path) = &self.handler {
            let mut path = PathBuf::from(file_path);

            if !path.is_absolute() {
                let exe_path = env::current_exe().unwrap();
                let cd = exe_path.parent().unwrap();
                path = cd.join(path);
            };

            let file = File::open(path).await;

            if file.is_err() {
                let err_kind = file.as_ref().err().unwrap().kind();
                return handle_file_error(_request, err_kind).await;
            }

            let file: File = file.unwrap();

            let mut builder = Response::builder().status(StatusCode::OK);

            let content_type = MIME_DICT.get_content_type(file_path.to_string());
            if content_type.is_some() {
                builder = builder.header(http::header::CONTENT_TYPE, content_type.unwrap());
            }

            let reader_stream = ReaderStream::new(file);

            // Convert to http_body_util::BoxBody
            let stream_body = StreamBody::new(reader_stream.map_ok(Frame::data));
            let boxed_body = stream_body.boxed();

            let response = builder.body(boxed_body).unwrap();
            response
        } else {
            unimplemented!(
                "Only file handler is supported. Given handler was {}",
                self.handler.type_name()
            )
        }
    }
}

async fn handle_file_error(
    _request: http::Request<impl hyper::body::Body>,
    error: ErrorKind,
) -> Response<BoxBody> {
    let handler = match error {
        ErrorKind::NotFound => RespondHandler {
            handler: types::Handler::Respond {
                status: Some(404),
                body: None,
            },
        },
        ErrorKind::PermissionDenied => RespondHandler {
            handler: types::Handler::Respond {
                status: Some(403),
                body: None,
            },
        },
        ErrorKind::IsADirectory => RespondHandler {
            handler: types::Handler::Respond {
                status: Some(403),
                body: None,
            },
        },
        _ => RespondHandler {
            handler: types::Handler::Respond {
                status: Some(500),
                body: None,
            },
        },
    };
    return handler.handle(_request).await;
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use chico_file::types;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;

    use crate::{
        handlers::{file::FileHandler, RequestHandler},
        test_utils::MockBody,
    };

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

        let file_handler = FileHandler {
            handler: types::Handler::File("index.html".to_string()),
        };

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

        let file_handler = FileHandler {
            handler: types::Handler::File(file_path.to_str().unwrap().to_string()),
        };

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
        let file_handler = FileHandler {
            handler: types::Handler::File("not-exist-index.html".to_string()),
        };

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
        let file_handler = FileHandler {
            handler: types::Handler::File(cd.to_str().unwrap().to_string()),
        };

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
}
