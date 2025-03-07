use std::{
    env,
    fs::File,
    io::{ErrorKind, Read},
    path::PathBuf,
};

use chico_file::types;
use http::{Response, StatusCode};
use http_body_util::Full;
use hyper::body::Bytes;

use crate::handlers::respond::RespondHandler;

use super::RequestHandler;

#[derive(PartialEq, Debug)]
pub struct FileHandler {
    pub handler: types::Handler,
}

impl RequestHandler for FileHandler {
    fn handle(
        &self,
        _request: hyper::Request<impl hyper::body::Body>,
    ) -> http::Response<http_body_util::Full<hyper::body::Bytes>> {
        if let types::Handler::File(file_path) = &self.handler {
            let mut path = PathBuf::from(file_path);

            if !path.is_absolute() {
                let exe_path = env::current_exe().unwrap();
                let cd = exe_path.parent().unwrap();
                println!("CD : {:?}", cd);
                path = cd.join(path);
            };

            let file = File::open(path);

            if file.is_err() {
                let handler = match file.err().unwrap().kind() {
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
                    _ => RespondHandler {
                        handler: types::Handler::Respond {
                            status: Some(500),
                            body: None,
                        },
                    },
                };
                return handler.handle(_request);
            }

            let mut file: File = file.unwrap();

            let mut buf = vec![];

            file.read_to_end(&mut buf).unwrap();

            Response::builder()
                .status(StatusCode::OK)
                .body(Full::new(Bytes::from_iter(buf)))
                .unwrap()
        } else {
            unimplemented!(
                "Only file handler is supported. Given handler was {}",
                self.handler.type_name()
            )
        }
    }
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

        let response = file_handler.handle(request);

        assert_eq!(&response.status(), &StatusCode::OK);
        let response_body = String::from_utf8(
            response
                .body()
                .clone()
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

        let response = file_handler.handle(request);

        assert_eq!(&response.status(), &StatusCode::OK);
        let response_body = String::from_utf8(
            response
                .body()
                .clone()
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

        let response = file_handler.handle(request);

        assert_eq!(&response.status(), &StatusCode::NOT_FOUND);
        let response_body = String::from_utf8(
            response
                .body()
                .clone()
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

        let response = file_handler.handle(request);

        assert_eq!(&response.status(), &StatusCode::FORBIDDEN);
        let response_body = String::from_utf8(
            response
                .body()
                .clone()
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
