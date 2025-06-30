use std::{str::FromStr, sync::Arc};

use crate::{config::ConfigExt, uri::UriExt, virtual_host::VirtualHostExt};
use chico_file::types::Config;
use file::FileHandler;
use http::Uri;
use hyper::{
    body::{Body, Bytes},
    Response,
};
use redirect::RedirectHandler;
use respond::RespondHandler;
pub type BoxBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

mod file;
mod redirect;
mod respond;
pub trait RequestHandler {
    async fn handle(&self, _request: &hyper::Request<impl Body>) -> Response<BoxBody>;
}

#[allow(dead_code)]
pub async fn handle_request(
    request: &hyper::Request<impl Body>,
    config: Arc<Config>,
) -> Response<BoxBody> {
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
    let vh = &config.find_virtual_host(host, port);

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

    let response = match &route.handler {
        chico_file::types::Handler::File(path) => {
            FileHandler::new(path.clone(), route.path.clone())
                .handle(request)
                .await
        }
        chico_file::types::Handler::Respond { status, body } => {
            RespondHandler::new(status.unwrap_or(200), body.clone())
                .handle(request)
                .await
        }
        chico_file::types::Handler::Redirect {
            path: _,
            status_code: _,
        } => {
            RedirectHandler {
                handler: route.handler.clone(),
            }
            .handle(request)
            .await
        }
        _ => todo!(),
    };

    response
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

        RespondHandler::not_found_with_body(body.to_string())
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
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use rstest::rstest;

    use crate::test_utils::MockBody;

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

        let response = handle_request(&request, Arc::new(config)).await;

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

        let response = handle_request(&request, Arc::new(config)).await;

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

        let response = handle_request(&request, Arc::new(config)).await;

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

        let response = handle_request(&request, Arc::new(config)).await;

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
