use std::{collections::HashMap, str::FromStr};

use chico_file::types::Config;
use crates_uri::UriExt;
use http::Uri;

use crate::{
    handlers::{
        file::FileHandler, redirect::RedirectHandler, respond::RespondHandler,
        reverse_proxy::ReverseProxyHandler,
    },
    load_balance::{node::Node, round_robin::RoundRobinBalancer, LoadBalance, SingleUpstream},
};

pub struct ServerPlan {
    virtual_hosts: HashMap<String, VirtualHostPlan>,
}

impl ServerPlan {
    pub fn find_virtual_host(&self, host: &str, port: u16) -> Option<&VirtualHostPlan> {
        //todo: do more advanced search and pattern matching for virtual host
        let vh = self.virtual_hosts.iter().find(|&vh| {
            Uri::from_str(&vh.1.domain).unwrap().host().unwrap() == host && vh.1.get_port() == port
        });
        match vh {
            Some((_, vhp)) => Some(vhp),
            None => None,
        }
    }
}

pub struct VirtualHostPlan {
    domain: String,
    routes: HashMap<String, RoutePlan>,
}

impl VirtualHostPlan {
    pub fn find_route(&self, path: &str) -> Option<&RoutePlan> {
        //todo: do more advanced search and pattern matching for request path
        let route = self.routes.iter().find(|&r| {
            if r.0.ends_with("/*") {
                let asterisk_index = r.0.rfind("*").unwrap();
                path.starts_with(&r.0[..asterisk_index])
            } else {
                r.0 == path
            }
        });

        match route {
            Some((_, plan)) => Some(plan),
            None => None,
        }
    }
    fn get_port(&self) -> u16 {
        Uri::from_str(&self.domain)
            .expect("Expected Valid host")
            .get_port()
    }
}

pub enum RoutePlan {
    File(FileHandler),
    Respond(RespondHandler),
    Redirect(RedirectHandler),
    ReverseProxy(ReverseProxyHandler),
}

impl ServerPlan {
    pub fn from_config(config: &Config) -> Self {
        let mut vhosts = HashMap::new();

        for vh in &config.virtual_hosts {
            let mut routes = HashMap::new();
            for r in &vh.routes {
                let handler = match &r.handler {
                    chico_file::types::Handler::File(path) => {
                        RoutePlan::File(FileHandler::new(path.clone(), r.path.clone()))
                    }
                    chico_file::types::Handler::Proxy(proxy_config) => {
                        let balancer: Box<dyn LoadBalance> = match &proxy_config.load_balancer {
                            chico_file::types::LoadBalancer::NoBalancer(upstream) => {
                                Box::new(SingleUpstream::new(Node::new(
                                    upstream.get_host_port().parse().unwrap(),
                                )))
                            }
                            chico_file::types::LoadBalancer::RoundRobin(upstreams) => {
                                Box::new(RoundRobinBalancer::new(
                                    upstreams
                                        .iter()
                                        .map(|u| Node::new(u.get_host_port().parse().unwrap()))
                                        .collect(),
                                ))
                            }
                        };
                        RoutePlan::ReverseProxy(ReverseProxyHandler::with_timeouts(
                            balancer,
                            proxy_config.request_timeout,
                            proxy_config.connection_timeout,
                        ))
                    }
                    chico_file::types::Handler::Dir(_) => todo!(),
                    chico_file::types::Handler::Browse(_) => todo!(),
                    chico_file::types::Handler::Respond { status, body } => {
                        RoutePlan::Respond(RespondHandler::new(status.unwrap_or(200), body.clone()))
                    }
                    chico_file::types::Handler::Redirect { path, status_code } => {
                        RoutePlan::Redirect(RedirectHandler::new(
                            path.clone()
                                .expect("path parameter for redirect handler exepted"),
                            *status_code,
                        ))
                    }
                };

                routes.insert(r.path.clone(), handler);
            }
            vhosts.insert(
                vh.domain.clone(),
                VirtualHostPlan {
                    domain: vh.domain.clone(),
                    routes,
                },
            );
        }

        ServerPlan {
            virtual_hosts: vhosts,
        }
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use claims::assert_some;
    use rstest::rstest;

    use crate::{
        handlers::file::FileHandler,
        plan::{RoutePlan, VirtualHostPlan},
    };

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
        let mut routes = HashMap::new();
        let route_plan = RoutePlan::File(FileHandler::new("".to_string(), path.to_string()));
        routes.insert(path.to_string(), route_plan);

        let virtual_hosts = VirtualHostPlan {
            domain: "".to_string(),
            routes,
        };

        let route = assert_some!(virtual_hosts.find_route(search_value));
        match route {
            RoutePlan::File(handler) => {
                assert_eq!(handler.path, "");
                assert_eq!(handler.route, path);
            }
            _ => {
                panic!("Unexpected route type")
            }
        }
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
        let mut routes = HashMap::new();
        let route_plan = RoutePlan::File(FileHandler::new("".to_string(), path.to_string()));
        routes.insert(path.to_string(), route_plan);

        let virtual_hosts = VirtualHostPlan {
            domain: "".to_string(),
            routes,
        };

        let route = virtual_hosts.find_route(search_value);
        assert!(route.is_none(), "Expected no route to be found");
    }
}
