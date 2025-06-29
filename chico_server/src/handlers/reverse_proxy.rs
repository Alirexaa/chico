// use hyper::body::Body;
// use hyper_util::rt::TokioIo;
// use tokio::net::TcpStream;

// use super::RequestHandler;

// #[derive(Debug)]
// pub struct ReverseProxyHandler<'a> {
//     incoming_request: &'a hyper::Request<()>,
//     upstream: String,
// }

// impl<'a> RequestHandler for ReverseProxyHandler<'a> {
//     async fn handle(&self, _request: hyper::Request<impl Body>) -> http::Response<super::BoxBody> {
//         todo!()
//     }
// }

// impl<'a> ReverseProxyHandler<'a> {
//     pub fn new(incoming_request: &'a hyper::Request<()>, upstream: String) -> Self {
//         Self {
//             incoming_request,
//             upstream,
//         }
//     }
// }
