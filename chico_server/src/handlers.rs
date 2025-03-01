use chico_file::types::Config;

use crate::{config::ConfigExt, virtual_host::VirtualHostExt};

#[allow(dead_code)]
pub trait RequestHandler {
    fn handle(_request: hyper::Request<()>) -> hyper::Response<()> {
        todo!();
    }
}

pub struct NullRequestHandler {}

impl RequestHandler for NullRequestHandler {
    fn handle(_request: hyper::Request<()>) -> hyper::Response<()> {
        std::todo!();
    }
}

#[allow(dead_code)]
pub fn select_handler(request: &hyper::Request<()>, config: Config) -> impl RequestHandler {
    //todo handle unwrap
    let host = request.headers().get(http::header::HOST).unwrap();
    let vh = &config.find_virtual_host(host.to_str().unwrap().to_string());

    if vh.is_none() {
        todo!("abort connection in this case");
    }

    let vh = vh.unwrap();

    let route = vh.find_route(request.uri().path().to_string());

    if route.is_none() {
        todo!("abort connection in this case");
    }

    let route = route.unwrap();

    _ = match route.handler {
        chico_file::types::Handler::File(_) => todo!(),
        chico_file::types::Handler::Proxy(_) => todo!(),
        chico_file::types::Handler::Dir(_) => todo!(),
        chico_file::types::Handler::Browse(_) => todo!(),
        chico_file::types::Handler::Respond { status: _, body: _ } => todo!(),
        chico_file::types::Handler::Redirect {
            path: _,
            status_code: _,
        } => todo!(),
    };

    #[allow(unreachable_code)]
    NullRequestHandler {}
}
