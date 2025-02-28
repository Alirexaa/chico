use chico_file::types;
use http::{status, Response};
use http_body_util::Full;
use hyper::body::Bytes;

use super::RequestHandler;

#[allow(dead_code)]
pub struct RespondHandler {
    pub handler: types::Handler,
}

impl RequestHandler for RespondHandler {
    fn handle(
        &self,
        _request: hyper::Request<hyper::body::Incoming>,
    ) -> hyper::Response<Full<Bytes>> {
        if let types::Handler::Respond { status, body } = &self.handler {
            let status = status.unwrap_or(status::StatusCode::OK.as_u16());
            let body = body.as_ref().unwrap_or(&"".to_string()).clone();

            Response::builder()
                .status(status)
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        } else {
            unimplemented!(
                "Only respond handler is supported. Given handler was {:}",
                self.handler.type_name()
            )
        }
    }
}
