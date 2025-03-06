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
