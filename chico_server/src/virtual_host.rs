use std::str::FromStr;

use chico_file::types::{Route, VirtualHost};
use http::Uri;

use crate::uri::UriExt;

pub trait VirtualHostExt {
    fn find_route(&self, path: &str) -> Option<&Route>;
    fn get_port(&self) -> u16;
}

impl VirtualHostExt for VirtualHost {
    fn find_route(&self, path: &str) -> Option<&Route> {
        //todo: do more advance search and pattern matching for request path
        let route = self.routes.iter().find(|&r| {
            if r.path.ends_with("/*") {
                let asterisk_index = r.path.rfind("*").unwrap();
                path.starts_with(&r.path[..asterisk_index])
            } else {
                r.path == path
            }
        });
        route
    }
    fn get_port(&self) -> u16 {
        Uri::from_str(&self.domain)
            .expect("Expected Valid host")
            .get_port()
    }
}

#[cfg(test)]
mod tests {

    use rstest::rstest;

    #[rstest]
    #[case("/", "/")]
    #[case("/blog", "/blog")]
    #[case("/blog/*", "/blog/post1")]
    #[case("/*", "/blog")]
    #[case("/*", "/blog/post1")]
    #[case("/*", "/api")]
    #[case("/*", "/api/products")]
    #[case("/*", "/api/products/get")]
    #[case("/api/*", "/api/products/get")]
    #[case("/api/products/*", "/api/products/get")]
    #[case("/api/products/get/*", "/api/products/get/1")]
    fn test_find_route_success(#[case] path: &str, #[case] search_value: &str) {
        use crate::virtual_host::VirtualHostExt;
        use chico_file::types::{Handler, Route, VirtualHost};

        let route1 = Route {
            handler: Handler::File("".to_string()),
            middlewares: vec![],
            path: path.to_string(),
        };

        let virtual_hosts = VirtualHost {
            domain: "".to_string(),
            routes: vec![route1.clone()],
        };

        assert_eq!(Some(&route1), virtual_hosts.find_route(search_value))
    }

    #[rstest]
    #[case("/", "/blog")]
    #[case("/blog", "/blog/posts")]
    #[case("/blog/", "/blog/posts")]
    #[case("/blog/posts", "/blog/posts/post1")]
    #[case("/api/prod", "/api/products")]
    #[case("/", "/api/products/get")]
    #[case("/api/products/*", "/api")]
    #[case("/api/products", "/api/products/get")]
    #[case("/api/products/get/*", "/api/products/get")]
    fn test_find_route_fail(#[case] path: &str, #[case] search_value: &str) {
        use crate::virtual_host::VirtualHostExt;
        use chico_file::types::{Handler, Route, VirtualHost};

        let route1 = Route {
            handler: Handler::File("".to_string()),
            middlewares: vec![],
            path: path.to_string(),
        };

        let virtual_hosts = VirtualHost {
            domain: "".to_string(),
            routes: vec![route1.clone()],
        };

        assert_eq!(None, virtual_hosts.find_route(search_value))
    }
}
