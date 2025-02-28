use chico_file::types::VirtualHost;
use respond::RespondHandler;

mod respond;

#[allow(dead_code)]
pub trait RequestHandler {
    fn handle(&self, _request: hyper::Request<()>) -> hyper::Response<()>;
}

pub struct NullRequestHandler {}

impl RequestHandler for NullRequestHandler {
    fn handle(&self, _request: hyper::Request<()>) -> hyper::Response<()> {
        std::todo!();
    }
}

#[allow(dead_code)]
pub fn select_handler(
    request: &hyper::Request<()>,
    vhs: Vec<VirtualHost>,
) -> Box<dyn RequestHandler> {
    //todo handle unwrap
    let host = request.headers().get(http::header::HOST).unwrap();
    let vh = vhs
        .iter()
        .find(|&vh| vh.domain == host.to_str().unwrap())
        .unwrap();

    let route = vh
        .routes
        .iter()
        .find(|&r| r.path == request.uri().path())
        .unwrap();

    let handler: Box<dyn RequestHandler> = match route.handler {
        chico_file::types::Handler::File(_) => Box::new(RespondHandler {
            handler: route.handler.clone(),
        }),
        chico_file::types::Handler::Proxy(_) => todo!(),
        chico_file::types::Handler::Dir(_) => todo!(),
        chico_file::types::Handler::Browse(_) => todo!(),
        chico_file::types::Handler::Respond { status: _, body: _ } => todo!(),
        chico_file::types::Handler::Redirect {
            path: _,
            status_code: _,
        } => todo!(),
    };

    handler
}
