use std::{sync::Arc, time::Duration};

use http::{HeaderValue, Uri};
use http_body_util::BodyExt;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tracing::{debug, error, info_span};

use crate::{
    handlers::{respond::RespondHandler, BoxBody, RequestHandler},
    load_balance::node::Node,
};

pub struct ReverseProxyHandler {
    load_balancer: Box<dyn crate::load_balance::LoadBalance>,
    request_timeout: Duration,
    connection_timeout: Duration,
}

#[allow(dead_code)]
impl ReverseProxyHandler {
    pub fn new(load_balancer: Box<dyn crate::load_balance::LoadBalance>) -> Self {
        Self { 
            load_balancer,
            request_timeout: Duration::from_secs(30),     // default 30 seconds
            connection_timeout: Duration::from_secs(10),  // default 10 seconds
        }
    }
    
    pub fn with_timeouts(
        load_balancer: Box<dyn crate::load_balance::LoadBalance>,
        request_timeout: Option<u64>,
        connection_timeout: Option<u64>,
    ) -> Self {
        Self {
            load_balancer,
            request_timeout: Duration::from_secs(request_timeout.unwrap_or(30)),
            connection_timeout: Duration::from_secs(connection_timeout.unwrap_or(10)),
        }
    }
    
    fn get_node(&self) -> Option<Arc<Node>> {
        self.load_balancer.get_node()
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
        let upstream = self.get_node().unwrap();
        let host_and_port = upstream.addr;
        
        // Apply connection timeout
        let connect_result = tokio::time::timeout(
            self.connection_timeout, 
            TcpStream::connect(host_and_port)
        ).await;

        let client_stream = match connect_result {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => {
                error!("could not connect to upstream server. Given upstream : {upstream} - Error : {error}" , upstream  = host_and_port, error= err);
                return RespondHandler::bad_gateway_with_body(
                    "502 Bad Gateway - could not connect to upstream server.".to_string(),
                )
                .handle(request)
                .await;
            }
            Err(_) => {
                error!("Connection timeout while connecting to upstream server: {}", host_and_port);
                return RespondHandler::bad_gateway_with_body(
                    "502 Bad Gateway - connection timeout to upstream server.".to_string(),
                )
                .handle(request)
                .await;
            }
        };
        debug!("connected to upstream");

        let io = TokioIo::new(client_stream);

        debug!("start handshake to upstream");
        let handshake_result = hyper::client::conn::http1::handshake(io).await;
        let (mut sender, conn) = match handshake_result {
            Ok(result) => result,
            Err(err) => {
                error!("Handshake with upstream server failed: {:?}", err);
                return RespondHandler::bad_gateway_with_body(
                    "502 Bad Gateway - handshake with upstream server failed.".to_string(),
                )
                .handle(request)
                .await;
            }
        };
        debug!("handshake-ed to upstream");

        tokio::task::spawn(async move {
            debug!("waiting for the connection");
            if let Err(err) = conn.await {
                error!("Connection failed: {:?}", err);
            }
            debug!("connection complated");
        });

        let scheme = "http";
        let path_and_query = request
            .uri()
            .path_and_query()
            .map(|x| x.as_str())
            .unwrap_or("/");

        let uri_string = format!("{scheme}://{host_and_port}{path_and_query}");

        let mut request = request;
        let uri = uri_string.parse::<Uri>().unwrap();
        let host_header = format!("{}:{}", &uri.host().unwrap(), &uri.port().unwrap());
        request.headers_mut().insert(
            http::header::HOST,
            HeaderValue::from_str(host_header.as_str()).unwrap(),
        );
        *request.uri_mut() = uri;

        debug!("start sending request");

        let timeout_result = tokio::time::timeout(self.request_timeout, sender.send_request(request)).await;

        let response = match timeout_result {
            Ok(Ok(response)) => response,
            Ok(Err(err)) => {
                error!("Error sending request to upstream: {:?}", err);
                return bad_gateway_response(
                    "502 Bad Gateway - error sending request.".to_string(),
                );
            }
            Err(_) => {
                error!("Timeout while sending request to upstream.");
                return gateway_timeout_response(
                    "504 Gateway Timeout - upstream did not respond in time.".to_string(),
                );
            }
        };

        debug!("request sent");
        debug!("start converting response");

        let (parts, body) = response.into_parts();
        let boxed_body = body.map_err(std::io::Error::other).boxed();
        debug!("response boxed");

        Response::from_parts(parts, boxed_body)
    }
}

fn bad_gateway_response(body: String) -> Response<BoxBody> {
    http::Response::builder()
        .status(502)
        .body(crate::handlers::full(body))
        .unwrap()
}

fn gateway_timeout_response(body: String) -> Response<BoxBody> {
    http::Response::builder()
        .status(504)
        .body(crate::handlers::full(body))
        .unwrap()
}
