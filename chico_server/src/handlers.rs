use chico_file::types::VirtualHost;

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
pub fn select_handler(request: &hyper::Request<()>, vhs: Vec<VirtualHost>) -> impl RequestHandler {
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
