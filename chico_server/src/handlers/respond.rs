use chico_file::types;
use http::{status, Response};

use super::RequestHandler;

#[allow(dead_code)]
pub struct RespondHandler {
    pub handler: types::Handler,
}

impl RequestHandler for RespondHandler {
    fn handle(&self, _request: hyper::Request<()>) -> hyper::Response<()> {
        if let types::Handler::Respond { status, body } = &self.handler {
            let status = status.unwrap_or(status::StatusCode::OK.as_u16());
            let _body = body.as_ref().unwrap_or(&"".to_string());

            Response::builder().status(status).body(()).unwrap()
        } else {
            unimplemented!(
                "Only respond handler is supported. Given handler was {:}",
                self.handler.type_name()
            )
        }
    }
}
