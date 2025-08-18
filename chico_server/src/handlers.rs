use std::{collections::HashMap, str::FromStr, sync::Arc};

use crate::{handlers::respond::RespondHandler, plan::ServerPlan};
use crates_uri::UriExt;
use http::{Request, Uri};
use hyper::{body::Bytes, Response};
pub type BoxBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

pub mod file;
pub mod redirect;
pub mod respond;
pub mod reverse_proxy;
pub trait RequestHandler {
    async fn handle<B>(&self, request: Request<B>) -> Response<BoxBody>
    where
        B: hyper::body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>;
}

#[allow(dead_code)]
pub async fn handle_request<B>(
    request: hyper::Request<B>,
    plan: Arc<ServerPlan>,
) -> Response<BoxBody>
where
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let host = request.headers().get(http::header::HOST);
    if host.is_none() {
        return UtilitiesResponses::bad_request_host_header_not_found_respond_handler()
            .handle(request)
            .await;
    }

    let host = host.unwrap().to_str();
    if host.is_err() {
        return UtilitiesResponses::bad_request_invalid_host_header_respond_handler()
            .handle(request)
            .await;
    }

    let host = host.unwrap();
    let uri = Uri::from_str(host);
    if uri.is_err() {
        return UtilitiesResponses::bad_request_invalid_host_header_respond_handler()
            .handle(request)
            .await;
    }

    let uri = uri.unwrap();
    let host = uri.host();
    if host.is_none() {
        return UtilitiesResponses::bad_request_invalid_host_header_respond_handler()
            .handle(request)
            .await;
    }

    let host = host.unwrap();
    let port = uri.get_port();
    let vh = &plan.find_virtual_host(host, port);

    if vh.is_none() {
        return UtilitiesResponses::not_found_respond_handler()
            .handle(request)
            .await;
    }

    let vh = vh.unwrap();

    let route = vh.find_route(request.uri().path());

    if route.is_none() {
        return UtilitiesResponses::not_found_respond_handler()
            .handle(request)
            .await;
    }

    let route = route.unwrap();

    match route {
        crate::plan::RoutePlan::File(h) => h.handle(request).await,
        crate::plan::RoutePlan::Respond(h) => h.handle(request).await,
        crate::plan::RoutePlan::Redirect(h) => h.handle(request).await,
        crate::plan::RoutePlan::ReverseProxy(h) => h.handle(request).await,
    }
}

pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    use http_body_util::{BodyExt, Full};
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

#[allow(dead_code)]
pub struct UtilitiesResponses;

#[allow(dead_code)]
impl UtilitiesResponses {
    pub fn not_found_respond_handler() -> RespondHandler {
        let body = r"<!DOCTYPE html>  
<html>  
<head>  
    <title>404 Not Found</title>  
</head>  
<body>  
    <h1>404 Not Found</h1>  
</body>  
</html>";

        let mut set_headers = HashMap::new();
        set_headers.insert(
            hyper::header::CONTENT_TYPE.to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        RespondHandler::with_headers(404, Some(body.to_string()), set_headers)
    }

    pub fn bad_request_host_header_not_found_respond_handler() -> RespondHandler {
        let body = "Host header is missing in the request.";
        RespondHandler::bad_request_with_body(String::from(body))
    }

    pub fn bad_request_invalid_host_header_respond_handler() -> RespondHandler {
        let body = "Invalid Host header.";
        RespondHandler::bad_request_with_body(String::from(body))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chico_file::types::{Config, Handler, Route, VirtualHost};
    use claims::assert_some;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use rstest::rstest;

    use crate::{plan::ServerPlan, test_utils::MockBody};

    use super::handle_request;

    #[tokio::test]
    async fn test_handle_request_should_return_not_found_when_given_route_not_configured() {
        let config = Config {
            virtual_hosts: vec![VirtualHost {
                domain: "localhost".to_string(),
                routes: vec![Route {
                    handler: Handler::File("index.html".to_string()),
                    path: "/".to_string(),
                    middlewares: vec![],
                }],
            }],
        };

        let request = Request::builder()
            .uri("http://localhost/blog")
            .header(http::header::HOST, "localhost")
            .body(MockBody::new(b""))
            .unwrap();

        let response = handle_request(request, Arc::new(ServerPlan::from_config(&config))).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        let body = r"<!DOCTYPE html>  
<html>  
<head>  
    <title>404 Not Found</title>  
</head>  
<body>  
    <h1>404 Not Found</h1>  
</body>  
</html>";
        assert_eq!(response_body, body);
    }

    #[tokio::test]
    async fn test_handle_request_should_return_not_found_when_host_not_configured() {
        let config = Config {
            virtual_hosts: vec![VirtualHost {
                domain: "localhost".to_string(),
                routes: vec![Route {
                    handler: Handler::File("index.html".to_string()),
                    path: "/".to_string(),
                    middlewares: vec![],
                }],
            }],
        };

        let request = Request::builder()
            .uri("http://localhost:3000/blog")
            .header(http::header::HOST, "localhost:3000")
            .body(MockBody::new(b""))
            .unwrap();

        let response = handle_request(request, Arc::new(ServerPlan::from_config(&config))).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_some!(
            response.headers().get(hyper::header::CONTENT_TYPE),
            "text/html; charset=utf-8"
        );
        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        let body = r"<!DOCTYPE html>  
<html>  
<head>  
    <title>404 Not Found</title>  
</head>  
<body>  
    <h1>404 Not Found</h1>  
</body>  
</html>";
        assert_eq!(response_body, body);
    }

    #[tokio::test]
    async fn test_select_handler_should_return_bad_request_respond_handler_when_host_header_not_provided(
    ) {
        let config = Config {
            virtual_hosts: vec![VirtualHost {
                domain: "localhost".to_string(),
                routes: vec![Route {
                    handler: Handler::File("index.html".to_string()),
                    path: "/".to_string(),
                    middlewares: vec![],
                }],
            }],
        };

        let request = Request::builder()
            .uri("http://localhost/blog")
            .body(MockBody::new(b""))
            .unwrap();

        let response = handle_request(request, Arc::new(ServerPlan::from_config(&config))).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        let body = r"Host header is missing in the request.";
        assert_eq!(response_body, body);
    }

    #[rstest]
    #[case("http://exa mple.com ")] // invalid host, contain space in hostname
    #[case("‎")] // invalid host, contain invisible ASCII code
    #[case("/blog")] // invalid host
    #[tokio::test]
    async fn test_select_handler_should_return_bad_request_respond_handler_when_host_is_not_valid(
        #[case] host_header: &str,
    ) {
        let config = Config {
            virtual_hosts: vec![VirtualHost {
                domain: "localhost".to_string(),
                routes: vec![Route {
                    handler: Handler::File("index.html".to_string()),
                    path: "/".to_string(),
                    middlewares: vec![],
                }],
            }],
        };

        let request = Request::builder()
            .uri("http://localhost/blog")
            .header(http::header::HOST, host_header)
            .body(MockBody::new(b""))
            .unwrap();

        let response = handle_request(request, Arc::new(ServerPlan::from_config(&config))).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response_body = String::from_utf8(
            response
                .boxed()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        let body = r"Invalid Host header.";
        assert_eq!(response_body, body);
    }
}
