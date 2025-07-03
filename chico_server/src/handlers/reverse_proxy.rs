use http::{HeaderValue, Uri};
use http_body_util::BodyExt;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tracing::{debug, error, info_span};

use crate::handlers::{respond::RespondHandler, RequestHandler};

#[derive(PartialEq, Debug)]
pub struct ReverseProxyHandler {
    upstream: String,
}

impl ReverseProxyHandler {
    pub fn new(upstream: String) -> Self {
        Self { upstream }
    }
}

impl RequestHandler for ReverseProxyHandler {
    async fn handle<B>(&self, request: Request<B>) -> Response<super::BoxBody>
    where
        B: hyper::body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let span = info_span!("my_span");
        let _guard = span.enter();
        debug!("start connect to upstream");
        let connect_result = TcpStream::connect(&self.upstream).await;

        let Ok(client_stream) = connect_result else {
            let err = connect_result.err().unwrap();
            error!("could not connect to upstream server. Given upstream : {upstream} - Error : {error}" , upstream  = &self.upstream, error= err);
            return RespondHandler::bad_gateway_with_body(
                "502 Bad Gateway - could not connect to upstream server.".to_string(),
            )
            .handle(request)
            .await;
        };

        let io = TokioIo::new(client_stream);
        debug!("connected to upstream");

        debug!("start handshake to upstream");
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();
        debug!("handshaked to upstream");

        tokio::task::spawn(async move {
            debug!("waiting for the connection");
            if let Err(err) = conn.await {
                error!("Connection failed: {:?}", err);
            }
            debug!("connection complated");
        });

        let uri_string = format!(
            "http://{}{}",
            &self.upstream,
            request
                .uri()
                .path_and_query()
                .map(|x| x.as_str())
                .unwrap_or("/")
        );

        // let (parts, body) = request.into_parts();
        // let mut forward_request = Request::from_parts(parts, body);
        let mut request = request;
        let uri = uri_string.parse::<Uri>().unwrap();
        let host_header = format!("{}:{}", &uri.host().unwrap(), &uri.port().unwrap());
        request.headers_mut().insert(
            http::header::HOST,
            HeaderValue::from_str(host_header.as_str()).unwrap(),
        );
        *request.uri_mut() = uri;

        debug!("start sending request");

        let response = sender.send_request(request).await.unwrap();

        debug!("request sent");
        debug!("start converting response");

        let (parts, body) = response.into_parts();
        let boxed_body = body
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            .boxed();
        debug!("response boxed");

        Response::from_parts(parts, boxed_body)
    }
}
