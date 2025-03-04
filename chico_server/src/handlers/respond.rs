use chico_file::types;
use http::{status, Response};
use http_body_util::Full;
use hyper::body::{Body, Bytes};

use super::RequestHandler;

pub struct RespondHandler {
    pub handler: types::Handler,
}

impl RequestHandler for RespondHandler {
    fn handle(&self, _request: hyper::Request<impl Body>) -> hyper::Response<Full<Bytes>> {
        if let types::Handler::Respond { status, body } = &self.handler {
            let status = status.unwrap_or(status::StatusCode::OK.as_u16());
            let body = body.as_ref().unwrap_or(&String::new()).clone();

            Response::builder()
                .status(status)
                .body(Full::new(Bytes::from(body)))
                .unwrap()
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

    use crate::handlers::RequestHandler;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use hyper::body::{Body, Bytes};

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
        let response = respond_handler.handle(request);

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
        assert_eq!(response.status(), StatusCode::OK);
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
        let response = respond_handler.handle(request);

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

        response.status();
        assert_eq!(response_body, "Hello, world!");
        assert_eq!(response.status(), StatusCode::OK);
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
        let response = respond_handler.handle(request);

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

        response.status();
        assert_eq!(response_body, "Access denied");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    struct MockBody {
        data: &'static [u8],
    }

    impl MockBody {
        fn new(data: &'static [u8]) -> Self {
            Self { data }
        }
    }

    impl Body for MockBody {
        type Data = Bytes;
        type Error = hyper::Error;

        fn poll_frame(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
            if self.data.is_empty() {
                std::task::Poll::Ready(None)
            } else {
                let data = self.data;
                self.data = &[];
                std::task::Poll::Ready(Some(Ok(hyper::body::Frame::data(Bytes::from(data)))))
            }
        }
    }
}
