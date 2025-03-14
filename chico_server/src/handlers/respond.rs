use chico_file::types;
use http::{status, Response};
use hyper::body::Body;

use super::{full, BoxBody, RequestHandler};

#[derive(PartialEq, Debug)]
pub struct RespondHandler {
    handler: types::Handler,
}

impl RespondHandler {
    #[allow(dead_code)]
    pub fn new(status: u16, body: Option<String>) -> RespondHandler {
        RespondHandler {
            handler: types::Handler::Respond {
                status: Some(status),
                body: (body),
            },
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
}

impl RequestHandler for RespondHandler {
    async fn handle(&self, _request: hyper::Request<impl Body>) -> hyper::Response<BoxBody> {
        if let types::Handler::Respond { status, body } = &self.handler {
            let status = status.unwrap_or(status::StatusCode::OK.as_u16());
            let body = body.as_ref().unwrap_or(&String::new()).clone();

            Response::builder().status(status).body(full(body)).unwrap()
        } else {
            unimplemented!(
                "Only respond handler is supported. Given handler was {}",
                self.handler.type_name()
            )
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{handlers::RequestHandler, test_utils::MockBody};
    use chico_file::types;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use rstest::rstest;

    use super::RespondHandler;

    #[tokio::test]
    async fn test_respond_handler_specified_status_no_body() {
        use super::RespondHandler;

        let respond_handler = RespondHandler {
            handler: chico_file::types::Handler::Respond {
                status: Some(200),
                body: None,
            },
        };

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
    async fn test_respond_handler_specified_body_no_status() {
        use super::RespondHandler;

        let respond_handler = RespondHandler {
            handler: chico_file::types::Handler::Respond {
                status: None,
                body: Some(String::from("Hello, world!")),
            },
        };

        let request_body: MockBody = MockBody::new(b"Hello, world!");

        let request = Request::builder().body(request_body).unwrap();
        let response = respond_handler.handle(request).await;

        assert_eq!(response.status(), StatusCode::OK);

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

        assert_eq!(response_body, "Hello, world!");
    }

    #[tokio::test]
    async fn test_respond_handler_specified_body_specified_status() {
        use super::RespondHandler;

        let respond_handler = RespondHandler {
            handler: chico_file::types::Handler::Respond {
                status: Some(403),
                body: Some(String::from("Access denied")),
            },
        };

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

    #[rstest]
    #[case(200, None,RespondHandler {
        handler: types::Handler::Respond {
            status: Some(200),
            body: None,
        },
    })]
    #[case(200, Some("OK".to_string()),RespondHandler {
        handler: types::Handler::Respond {
            status: Some(200),
            body: Some("OK".to_string()),
        },
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
