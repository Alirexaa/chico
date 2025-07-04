use std::collections::HashMap;

use http::Response;

use super::{full, RequestHandler};

#[derive(PartialEq, Debug)]
pub struct RespondHandler {
    status: u16,
    body: Option<String>,
    set_headers: HashMap<String, String>,
}

impl RespondHandler {
    #[allow(dead_code)]
    pub fn new(status: u16, body: Option<String>) -> RespondHandler {
        RespondHandler {
            status,
            body,
            set_headers: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_headers(
        status: u16,
        body: Option<String>,
        set_headers: HashMap<String, String>,
    ) -> RespondHandler {
        RespondHandler {
            status,
            body,
            set_headers,
        }
    }

    #[allow(dead_code)]
    pub fn ok() -> RespondHandler {
        RespondHandler::new(200, None)
    }

    #[allow(dead_code)]
    pub fn ok_with_body(body: String) -> RespondHandler {
        RespondHandler::new(200, Some(body))
    }

    #[allow(dead_code)]
    pub fn bad_request() -> RespondHandler {
        RespondHandler::new(400, None)
    }

    #[allow(dead_code)]
    pub fn bad_request_with_body(body: String) -> RespondHandler {
        RespondHandler::new(400, Some(body))
    }

    #[allow(dead_code)]
    pub fn not_found() -> RespondHandler {
        RespondHandler::new(404, None)
    }

    #[allow(dead_code)]
    pub fn not_found_with_body(body: String) -> RespondHandler {
        RespondHandler::new(404, Some(body))
    }

    #[allow(dead_code)]
    pub fn forbidden() -> RespondHandler {
        RespondHandler::new(403, None)
    }

    #[allow(dead_code)]
    pub fn forbidden_with_body(body: String) -> RespondHandler {
        RespondHandler::new(403, Some(body))
    }

    #[allow(dead_code)]
    pub fn internal_server_error() -> RespondHandler {
        RespondHandler::new(500, None)
    }

    #[allow(dead_code)]
    pub fn internal_server_error_with_body(body: String) -> RespondHandler {
        RespondHandler::new(500, Some(body))
    }

    #[allow(dead_code)]
    pub fn range_not_satisfiable() -> RespondHandler {
        RespondHandler::new(416, None)
    }

    #[allow(dead_code)]
    pub fn bad_gateway() -> RespondHandler {
        RespondHandler::new(416, None)
    }

    #[allow(dead_code)]
    pub fn bad_gateway_with_body(body: String) -> RespondHandler {
        RespondHandler::new(502, Some(body))
    }
}

impl RequestHandler for RespondHandler {
    async fn handle<B>(&self, _request: hyper::Request<B>) -> Response<super::BoxBody>
    where
        B: hyper::body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let body = self.body.as_ref().unwrap_or(&String::new()).clone();

        let mut builder = Response::builder().status(self.status);
        for (key, value) in &self.set_headers {
            builder = builder.header(key, value);
        }

        builder.body(full(body)).unwrap()
    }
}

#[cfg(test)]
mod tests {

    use crate::{handlers::RequestHandler, test_utils::MockBody};
    use claims::assert_some;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use rstest::rstest;
    use std::collections::HashMap;

    use super::RespondHandler;

    #[tokio::test]
    async fn test_respond_handler_specified_status_no_body() {
        use super::RespondHandler;

        let respond_handler = RespondHandler::new(200, None);

        let request_body: MockBody = MockBody::new(b"");

        let request = Request::builder().body(request_body).unwrap();
        let response = respond_handler.handle(request).await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();

        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn test_respond_handler_specified_body_specified_status() {
        use super::RespondHandler;

        let respond_handler = RespondHandler::new(403, Some(String::from("Access denied")));

        let request_body: MockBody = MockBody::new(b"Access denied");

        let request = Request::builder().body(request_body).unwrap();
        let response = respond_handler.handle(request).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

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

        assert_eq!(response_body, "Access denied");
    }

    #[tokio::test]
    async fn test_respond_handler_with_headers() {
        use super::RespondHandler;

        let mut set_headers = HashMap::new();
        set_headers.insert("Header-Key-1".to_string(), "value-1".to_string());
        set_headers.insert("Header-Key-2".to_string(), "value-2".to_string());
        let respond_handler =
            RespondHandler::with_headers(200, Some(String::from("Everything is OK")), set_headers);

        let request_body: MockBody = MockBody::new(b"Everything is OK");

        let request = Request::builder().body(request_body).unwrap();
        let response = respond_handler.handle(request).await;

        assert_eq!(response.status(), StatusCode::OK);

        assert_some!(
            response.headers().get("Header-Key-1".to_string()),
            "value-1"
        );

        assert_some!(
            response.headers().get("Header-Key-2".to_string()),
            "value-2"
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

        assert_eq!(response_body, "Everything is OK");
    }

    #[rstest]
    #[case(200, None,RespondHandler {
        status: 200,
        body : None,
        set_headers : HashMap::new()
    })]
    #[case(200, Some("OK".to_string()),RespondHandler {
       status: 200,
       body: Some("OK".to_string()),
       set_headers : HashMap::new()

    })]
    fn test_respond_handler_new(
        #[case] status: u16,
        #[case] body: Option<String>,
        #[case] result: RespondHandler,
    ) {
        let handler = RespondHandler::new(status, body);
        assert_eq!(result, handler);
    }

    #[test]
    fn test_respond_handler_ok() {
        let handler = RespondHandler::ok();
        assert_eq!(RespondHandler::new(200, None), handler);
    }
    #[test]
    fn test_respond_handler_ok_with_body() {
        let handler = RespondHandler::ok_with_body("Ok".to_string());
        assert_eq!(RespondHandler::new(200, Some("Ok".to_string())), handler);
    }

    #[test]
    fn test_respond_handler_bad_request() {
        let handler = RespondHandler::bad_request();
        assert_eq!(RespondHandler::new(400, None), handler);
    }
    #[test]
    fn test_respond_handler_bad_request_with_body() {
        let handler = RespondHandler::bad_request_with_body("Bad Request".to_string());
        assert_eq!(
            RespondHandler::new(400, Some("Bad Request".to_string())),
            handler
        );
    }

    #[test]
    fn test_respond_handler_forbidden() {
        let handler = RespondHandler::forbidden();
        assert_eq!(RespondHandler::new(403, None), handler);
    }
    #[test]
    fn test_respond_handler_forbidden_with_body() {
        let handler = RespondHandler::forbidden_with_body("Forbidden".to_string());
        assert_eq!(
            RespondHandler::new(403, Some("Forbidden".to_string())),
            handler
        );
    }

    #[test]
    fn test_respond_handler_not_found() {
        let handler = RespondHandler::not_found();
        assert_eq!(RespondHandler::new(404, None), handler);
    }

    #[test]
    fn test_respond_handler_not_found_with_body() {
        let handler = RespondHandler::not_found_with_body("Not Found".to_string());
        assert_eq!(
            RespondHandler::new(404, Some("Not Found".to_string())),
            handler
        );
    }

    #[test]
    fn test_respond_handler_internal_server_error() {
        let handler = RespondHandler::internal_server_error();
        assert_eq!(RespondHandler::new(500, None), handler);
    }

    #[test]
    fn test_respond_handler_internal_server_error_with_body() {
        let handler =
            RespondHandler::internal_server_error_with_body("Internal Server Error".to_string());
        assert_eq!(
            RespondHandler::new(500, Some("Internal Server Error".to_string())),
            handler
        );
    }
}
