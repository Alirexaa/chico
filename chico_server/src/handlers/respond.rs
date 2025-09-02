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

    /// Creates an HTML error response with proper formatting and content-type header
    #[allow(dead_code)]
    fn create_html_error_response(
        status_code: u16,
        error_title: &str,
        error_message: &str,
    ) -> RespondHandler {
        let body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{} {}</title>
</head>
<body>
    <h1>{} {}</h1>
    <p>{}</p>
</body>
</html>"#,
            status_code, error_title, status_code, error_title, error_message
        );

        let mut set_headers = HashMap::new();
        set_headers.insert(
            hyper::header::CONTENT_TYPE.to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        RespondHandler::with_headers(status_code, Some(body), set_headers)
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
        Self::create_html_error_response(
            400,
            "Bad Request",
            "The request could not be understood by the server.",
        )
    }

    #[allow(dead_code)]
    pub fn bad_request_with_body(body: String) -> RespondHandler {
        Self::create_html_error_response(400, "Bad Request", &body)
    }

    #[allow(dead_code)]
    pub fn not_found() -> RespondHandler {
        Self::create_html_error_response(
            404,
            "Not Found",
            "The requested resource could not be found on this server.",
        )
    }

    #[allow(dead_code)]
    pub fn not_found_with_body(body: String) -> RespondHandler {
        Self::create_html_error_response(404, "Not Found", &body)
    }

    #[allow(dead_code)]
    pub fn forbidden() -> RespondHandler {
        Self::create_html_error_response(
            403,
            "Forbidden",
            "You don't have permission to access this resource.",
        )
    }

    #[allow(dead_code)]
    pub fn forbidden_with_body(body: String) -> RespondHandler {
        Self::create_html_error_response(403, "Forbidden", &body)
    }

    #[allow(dead_code)]
    pub fn internal_server_error() -> RespondHandler {
        Self::create_html_error_response(
            500,
            "Internal Server Error",
            "The server encountered an internal error and was unable to complete your request.",
        )
    }

    #[allow(dead_code)]
    pub fn internal_server_error_with_body(body: String) -> RespondHandler {
        Self::create_html_error_response(500, "Internal Server Error", &body)
    }

    #[allow(dead_code)]
    pub fn range_not_satisfiable() -> RespondHandler {
        Self::create_html_error_response(
            416,
            "Range Not Satisfiable",
            "The requested range cannot be satisfied.",
        )
    }

    #[allow(dead_code)]
    pub fn bad_gateway() -> RespondHandler {
        Self::create_html_error_response(
            502,
            "Bad Gateway",
            "The server received an invalid response from an upstream server.",
        )
    }

    #[allow(dead_code)]
    pub fn bad_gateway_with_body(body: String) -> RespondHandler {
        Self::create_html_error_response(502, "Bad Gateway", &body)
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
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>400 Bad Request</title>
</head>
<body>
    <h1>400 Bad Request</h1>
    <p>The request could not be understood by the server.</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(400, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }
    #[test]
    fn test_respond_handler_bad_request_with_body() {
        let handler = RespondHandler::bad_request_with_body("Bad Request".to_string());
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>400 Bad Request</title>
</head>
<body>
    <h1>400 Bad Request</h1>
    <p>Bad Request</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(400, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }

    #[test]
    fn test_respond_handler_forbidden() {
        let handler = RespondHandler::forbidden();
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>403 Forbidden</title>
</head>
<body>
    <h1>403 Forbidden</h1>
    <p>You don't have permission to access this resource.</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(403, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }
    #[test]
    fn test_respond_handler_forbidden_with_body() {
        let handler = RespondHandler::forbidden_with_body("Forbidden".to_string());
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>403 Forbidden</title>
</head>
<body>
    <h1>403 Forbidden</h1>
    <p>Forbidden</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(403, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }

    #[test]
    fn test_respond_handler_not_found() {
        let handler = RespondHandler::not_found();
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>404 Not Found</title>
</head>
<body>
    <h1>404 Not Found</h1>
    <p>The requested resource could not be found on this server.</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(404, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }

    #[test]
    fn test_respond_handler_not_found_with_body() {
        let handler = RespondHandler::not_found_with_body("Not Found".to_string());
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>404 Not Found</title>
</head>
<body>
    <h1>404 Not Found</h1>
    <p>Not Found</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(404, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }

    #[test]
    fn test_respond_handler_internal_server_error() {
        let handler = RespondHandler::internal_server_error();
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>500 Internal Server Error</title>
</head>
<body>
    <h1>500 Internal Server Error</h1>
    <p>The server encountered an internal error and was unable to complete your request.</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(500, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }

    #[test]
    fn test_respond_handler_internal_server_error_with_body() {
        let handler =
            RespondHandler::internal_server_error_with_body("Internal Server Error".to_string());
        let expected_body = "<!DOCTYPE html>
<html>
<head>
    <title>500 Internal Server Error</title>
</head>
<body>
    <h1>500 Internal Server Error</h1>
    <p>Internal Server Error</p>
</body>
</html>";
        let mut expected_headers = HashMap::new();
        expected_headers.insert(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let expected =
            RespondHandler::with_headers(500, Some(expected_body.to_string()), expected_headers);
        assert_eq!(expected, handler);
    }

    /// Test to verify that all error responses are properly unified with HTML format
    #[tokio::test]
    async fn test_all_error_responses_have_html_content_type() {
        let error_handlers = vec![
            ("400 Bad Request", RespondHandler::bad_request()),
            ("403 Forbidden", RespondHandler::forbidden()),
            ("404 Not Found", RespondHandler::not_found()),
            ("500 Internal Server Error", RespondHandler::internal_server_error()),
            ("502 Bad Gateway", RespondHandler::bad_gateway()),
            ("416 Range Not Satisfiable", RespondHandler::range_not_satisfiable()),
        ];

        for (name, handler) in error_handlers {
            let request = Request::builder().body(MockBody::new(b"")).unwrap();
            let response = handler.handle(request).await;

            // Check that content-type header is set correctly
            let content_type = response.headers().get("content-type").unwrap();
            assert_eq!(content_type, "text/html; charset=utf-8", "Failed for {}", name);

            // Check that the response body contains HTML
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
            
            assert!(body.contains("<!DOCTYPE html>"), "Missing DOCTYPE for {}", name);
            assert!(body.contains("<html>"), "Missing html tag for {}", name);
            assert!(body.contains("<head>"), "Missing head tag for {}", name);
            assert!(body.contains("<title>"), "Missing title tag for {}", name);
            assert!(body.contains("</title>"), "Missing closing title tag for {}", name);
            assert!(body.contains("</head>"), "Missing closing head tag for {}", name);
            assert!(body.contains("<body>"), "Missing body tag for {}", name);
            assert!(body.contains("</body>"), "Missing closing body tag for {}", name);
            assert!(body.contains("</html>"), "Missing closing html tag for {}", name);
        }
    }

    #[tokio::test]
    async fn test_error_responses_include_status_code_in_title() {
        let test_cases = vec![
            (RespondHandler::bad_request(), "400 Bad Request"),
            (RespondHandler::forbidden(), "403 Forbidden"),
            (RespondHandler::not_found(), "404 Not Found"),
            (RespondHandler::internal_server_error(), "500 Internal Server Error"),
            (RespondHandler::bad_gateway(), "502 Bad Gateway"),
        ];

        for (handler, expected_title) in test_cases {
            let request = Request::builder().body(MockBody::new(b"")).unwrap();
            let response = handler.handle(request).await;

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

            assert!(body.contains(&format!("<title>{}</title>", expected_title)), 
                    "Title should contain '{}' but got body: {}", expected_title, body);
            assert!(body.contains(&format!("<h1>{}</h1>", expected_title)), 
                    "H1 should contain '{}' but got body: {}", expected_title, body);
        }
    }
}
