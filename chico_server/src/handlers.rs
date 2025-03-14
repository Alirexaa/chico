use std::sync::Arc;

use chico_file::types::Config;
use file::FileHandler;
use redirect::RedirectHandler;

use crate::{config::ConfigExt, virtual_host::VirtualHostExt};
use hyper::{
    body::{Body, Bytes},
    Response,
};
use respond::RespondHandler;
pub type BoxBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

mod file;
mod redirect;
mod respond;
pub trait RequestHandler {
    async fn handle(&self, _request: hyper::Request<impl Body>) -> Response<BoxBody>;
}

#[allow(dead_code)]
pub fn select_handler(request: &hyper::Request<impl Body>, config: Arc<Config>) -> HandlerEnum {
    let host = request.headers().get(http::header::HOST);

    if host.is_none() {
        return HandlerEnum::bad_request_host_header_not_found_respond_handler();
    }

    let host = host.unwrap();

    let vh = &config.find_virtual_host(host.to_str().unwrap());

    if vh.is_none() {
        return HandlerEnum::not_found_respond_handler();
    }

    let vh = vh.unwrap();

    let route = vh.find_route(request.uri().path());

    if route.is_none() {
        return HandlerEnum::not_found_respond_handler();
    }

    let route = route.unwrap();

    let handler: HandlerEnum = match &route.handler {
        chico_file::types::Handler::File(_) => HandlerEnum::File(FileHandler {
            handler: route.handler.clone(),
        }),
        chico_file::types::Handler::Proxy(_) => todo!(),
        chico_file::types::Handler::Dir(_) => todo!(),
        chico_file::types::Handler::Browse(_) => todo!(),
        chico_file::types::Handler::Respond { status, body } => {
            HandlerEnum::Respond(RespondHandler::new(status.unwrap_or(200), body.clone()))
        }
        chico_file::types::Handler::Redirect {
            path: _,
            status_code: _,
        } => HandlerEnum::Redirect(RedirectHandler {
            handler: route.handler.clone(),
        }),
    };

    handler
}

pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    use http_body_util::{BodyExt, Full};
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

#[derive(PartialEq, Debug)]
pub enum HandlerEnum {
    #[allow(dead_code)]
    Respond(RespondHandler),
    Redirect(RedirectHandler),
    File(FileHandler),
}

impl RequestHandler for HandlerEnum {
    async fn handle(&self, request: hyper::Request<impl Body>) -> Response<BoxBody> {
        match self {
            HandlerEnum::Respond(handler) => handler.handle(request).await,
            HandlerEnum::Redirect(handler) => handler.handle(request).await,
            HandlerEnum::File(handler) => handler.handle(request).await,
        }
    }
}

impl HandlerEnum {
    pub fn not_found_respond_handler() -> HandlerEnum {
        let body = r"<!DOCTYPE html>  
<html>  
<head>  
    <title>404 Not Found</title>  
</head>  
<body>  
    <h1>404 Not Found</h1>  
</body>  
</html>";

        HandlerEnum::Respond(RespondHandler::not_found_with_body(body.to_string()))
    }

    pub fn bad_request_host_header_not_found_respond_handler() -> HandlerEnum {
        let body = "Host header is missing in the request.";
        HandlerEnum::Respond(RespondHandler::bad_request_with_body(String::from(body)))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chico_file::types::{Config, Handler, Route, VirtualHost};
    use http::Request;

    use crate::{handlers::HandlerEnum, test_utils::MockBody};

    use super::{respond::RespondHandler, select_handler};

    #[test]
    fn test_select_handler_should_return_not_found_respond_handler_when_given_route_not_configured()
    {
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

        let handler = select_handler(&request, Arc::new(config));

        assert_eq!(HandlerEnum::not_found_respond_handler(), handler)
    }

    #[test]
    fn test_select_handler_should_return_not_found_respond_handler_when_host_not_configured() {
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

        let handler = select_handler(&request, Arc::new(config));

        assert_eq!(HandlerEnum::not_found_respond_handler(), handler)
    }

    #[test]
    fn test_handler_enum_not_found_respond_handler() {
        let body = r"<!DOCTYPE html>  
<html>  
<head>  
    <title>404 Not Found</title>  
</head>  
<body>  
    <h1>404 Not Found</h1>  
</body>  
</html>";

        let handler = HandlerEnum::Respond(RespondHandler::not_found_with_body(body.to_string()));

        assert_eq!(handler, HandlerEnum::not_found_respond_handler())
    }

    #[test]
    fn test_handler_enum_bad_request_host_header_not_found_respond_handler() {
        let body = "Host header is missing in the request.";

        let handler =
            HandlerEnum::Respond(RespondHandler::bad_request_with_body(String::from(body)));

        assert_eq!(
            handler,
            HandlerEnum::bad_request_host_header_not_found_respond_handler()
        )
    }

    #[test]
    fn test_select_handler_should_return_not_bad_request_respond_handler_when_host_header_not_provided(
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

        let handler = select_handler(&request, Arc::new(config));

        assert_eq!(
            HandlerEnum::bad_request_host_header_not_found_respond_handler(),
            handler
        )
    }
}
