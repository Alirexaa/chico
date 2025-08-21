use http::{Response, StatusCode};

use super::{full, RequestHandler};

#[derive(PartialEq, Debug)]
pub struct RedirectHandler {
    path: String,
    status_code: Option<u16>,
}

impl RedirectHandler {
    pub fn new(path: String, status_code: Option<u16>) -> Self {
        Self { path, status_code }
    }
}

impl RequestHandler for RedirectHandler {
    async fn handle<B>(&self, _request: hyper::Request<B>) -> Response<super::BoxBody>
    where
        B: hyper::body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let path = &self.path;

        let status_code = self.status_code.unwrap_or(StatusCode::FOUND.as_u16());

        Response::builder()
            .status(status_code)
            .header(http::header::LOCATION, path)
            .body(full(""))
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use http::{Request, StatusCode};

    use crate::{handlers::RequestHandler, test_utils::MockBody};

    use super::RedirectHandler;

    #[tokio::test]
    async fn test_redirect_handler_not_specified_status() {
        let redirect_handler = RedirectHandler::new("/new-path".to_string(), None);

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = redirect_handler.handle(request).await;

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

    #[tokio::test]
    async fn test_redirect_handler_specified_status() {
        let redirect_handler = RedirectHandler::new("/new-path".to_string(), Some(307));

        let request_body: MockBody = MockBody::new(b"");
        let request = Request::builder().body(request_body).unwrap();

        let response = redirect_handler.handle(request).await;

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
