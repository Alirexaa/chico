use chico_file::types;
use http::{Response, StatusCode};
use http_body_util::Full;

use super::RequestHandler;

pub struct RedirectHandler {
    pub handler: types::Handler,
}

impl RequestHandler for RedirectHandler {
    fn handle(
        &self,
        _request: hyper::Request<impl hyper::body::Body>,
    ) -> http::Response<http_body_util::Full<hyper::body::Bytes>> {
        if let types::Handler::Redirect { path, status_code } = &self.handler {
            // Based on chico file path is always some
            let path = path.clone().expect("Expected path value not provided.");
            let status_code = status_code.unwrap_or(StatusCode::FOUND.as_u16());

            Response::builder()
                .status(status_code)
                .header(http::header::LOCATION, path)
                .body(Full::default())
                .unwrap()
        } else {
            unimplemented!(
                "Only redirect handler is supported. Given handler was {}",
                self.handler.type_name()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use chico_file::types;
    use http::{Request, StatusCode};

    use crate::{handlers::RequestHandler, test_utils::MockBody};

    use super::RedirectHandler;

    #[test]
    fn test_redirect_handler_not_specified_status() {
        let redirect_handler = RedirectHandler {
            handler: types::Handler::Redirect {
                path: Some("/new-path".to_string()),
                status_code: None,
            },
        };

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = redirect_handler.handle(request);

        assert_eq!(&response.status(), &StatusCode::FOUND);
        assert_eq!(
            response
                .headers()
                .get(http::header::LOCATION)
                .expect("Expected Location header not provided.")
                .to_str()
                .unwrap(),
            "/new-path".to_string()
        );
    }

    #[test]
    fn test_redirect_handler_specified_status() {
        let redirect_handler = RedirectHandler {
            handler: types::Handler::Redirect {
                path: Some("/new-path".to_string()),
                status_code: Some(307),
            },
        };

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = redirect_handler.handle(request);

        assert_eq!(&response.status(), &StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get(http::header::LOCATION)
                .expect("Expected Location header not provided.")
                .to_str()
                .unwrap(),
            "/new-path".to_string()
        );
    }
}
