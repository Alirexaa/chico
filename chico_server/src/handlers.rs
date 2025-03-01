use chico_file::types::Config;

use crate::{config::ConfigExt, virtual_host::VirtualHostExt};
use http_body_util::Full;
use hyper::{body::Bytes, Response};
use respond::RespondHandler;

mod respond;

#[allow(dead_code)]
pub trait RequestHandler {
    fn handle(&self, _request: hyper::Request<hyper::body::Incoming>) -> Response<Full<Bytes>>;
}

pub struct NullRequestHandler {}

impl RequestHandler for NullRequestHandler {
    fn handle(&self, _request: hyper::Request<hyper::body::Incoming>) -> Response<Full<Bytes>> {
        todo!()
    }
}

#[allow(dead_code)]
pub fn select_handler(
    request: &hyper::Request<hyper::body::Incoming>,
    config: Config,
) -> Box<dyn RequestHandler> {
    //todo handle unwrap
    let host = request.headers().get(http::header::HOST).unwrap();
    println!("host: {:?}", host);
    let vh = &config.find_virtual_host(host.to_str().unwrap());

    if vh.is_none() {
        todo!("abort connection in this case");
    }

    let vh = vh.unwrap();

    let route = vh.find_route(request.uri().path());

    if route.is_none() {
        todo!("abort connection in this case");
    }

    let route = route.unwrap();

    let handler: Box<dyn RequestHandler> = match route.handler {
        chico_file::types::Handler::File(_) => todo!(),
        chico_file::types::Handler::Proxy(_) => todo!(),
        chico_file::types::Handler::Dir(_) => todo!(),
        chico_file::types::Handler::Browse(_) => todo!(),
        chico_file::types::Handler::Respond { status: _, body: _ } => Box::new(RespondHandler {
            handler: route.handler.clone(),
        }),
        chico_file::types::Handler::Redirect {
            path: _,
            status_code: _,
        } => todo!(),
    };

    handler
}
