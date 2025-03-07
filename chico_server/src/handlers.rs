use chico_file::types::Config;
use file::FileHandler;
use redirect::RedirectHandler;

use crate::{config::ConfigExt, virtual_host::VirtualHostExt};
use http_body_util::Full;
use hyper::{
    body::{Body, Bytes},
    Response,
};
use respond::RespondHandler;

mod file;
mod redirect;
mod respond;
pub trait RequestHandler {
    fn handle(&self, _request: hyper::Request<impl Body>) -> Response<Full<Bytes>>;
}

#[derive(PartialEq, Debug)]
pub struct NullRequestHandler {}

impl RequestHandler for NullRequestHandler {
    fn handle(&self, _request: hyper::Request<impl Body>) -> Response<Full<Bytes>> {
        todo!()
    }
}

#[allow(dead_code)]
pub fn select_handler(request: &hyper::Request<impl Body>, config: Config) -> HandlerEnum {
    //todo handle unwrap
    let host = request.headers().get(http::header::HOST).unwrap();
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

    let handler: HandlerEnum = match route.handler {
        chico_file::types::Handler::File(_) => HandlerEnum::File(FileHandler {
            handler: route.handler.clone(),
        }),
        chico_file::types::Handler::Proxy(_) => todo!(),
        chico_file::types::Handler::Dir(_) => todo!(),
        chico_file::types::Handler::Browse(_) => todo!(),
        chico_file::types::Handler::Respond { status: _, body: _ } => {
            HandlerEnum::Respond(RespondHandler {
                handler: route.handler.clone(),
            })
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

#[derive(PartialEq, Debug)]
pub enum HandlerEnum {
    #[allow(dead_code)]
    Null(NullRequestHandler),
    Respond(RespondHandler),
    Redirect(RedirectHandler),
    File(FileHandler),
}

impl RequestHandler for HandlerEnum {
    fn handle(&self, request: hyper::Request<impl Body>) -> Response<Full<Bytes>> {
        match self {
            HandlerEnum::Null(handler) => handler.handle(request),
            HandlerEnum::Respond(handler) => handler.handle(request),
            HandlerEnum::Redirect(handler) => handler.handle(request),
            HandlerEnum::File(handler) => handler.handle(request),
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

        HandlerEnum::Respond(RespondHandler {
            handler: chico_file::types::Handler::Respond {
                status: Some(404),
                body: Some(body.to_string()),
            },
        })
    }
}

#[cfg(test)]
mod tests {
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

        let handler = select_handler(&request, config);

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

        let handler = select_handler(&request, config);

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

        let handler = HandlerEnum::Respond(RespondHandler {
            handler: chico_file::types::Handler::Respond {
                status: Some(404),
                body: Some(body.to_string()),
            },
        });

        assert_eq!(handler, HandlerEnum::not_found_respond_handler())
    }
}
