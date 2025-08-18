use std::{collections::HashMap, str::FromStr};

use chico_file::types::Config;
use crates_uri::UriExt;
use http::Uri;

use crate::{
    handlers::{
        file::FileHandler, redirect::RedirectHandler, respond::RespondHandler,
        reverse_proxy::ReverseProxyHandler,
    },
    load_balance::{node::Node, round_robin::RoundRobinBalancer, SingleUpstream},
};

pub struct ServerPlan {
    virtual_hosts: HashMap<String, VirtualHostPlan>,
}

impl ServerPlan {
    pub fn find_virtual_host(&self, host: &str, port: u16) -> Option<&VirtualHostPlan> {
        //todo: do more advance search and pattern matching for virtual host
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
        //todo: do more advance search and pattern matching for request path
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
                    chico_file::types::Handler::Proxy(load_balancer) => match load_balancer {
                        chico_file::types::LoadBalancer::NoBalancer(upstream) => {
                            let balancer = SingleUpstream::new(Node::new(
                                upstream.get_host_port().parse().unwrap(),
                            ));
                            RoutePlan::ReverseProxy(ReverseProxyHandler::new(Box::new(balancer)))
                        }
                        chico_file::types::LoadBalancer::RoundRobin(upstreams) => {
                            let balancer = RoundRobinBalancer::new(
                                upstreams
                                    .iter()
                                    .map(|u| Node::new(u.get_host_port().parse().unwrap()))
                                    .collect(),
                            );
                            RoutePlan::ReverseProxy(ReverseProxyHandler::new(Box::new(balancer)))
                        }
                    },
                    chico_file::types::Handler::Dir(_) => todo!(),
                    chico_file::types::Handler::Browse(_) => todo!(),
                    chico_file::types::Handler::Respond { status, body } => {
                        RoutePlan::Respond(RespondHandler::new(status.unwrap_or(200), body.clone()))
                    }
                    chico_file::types::Handler::Redirect { path, status_code } => {
                        RoutePlan::Redirect(RedirectHandler::new(
                            path.clone()
                                .expect("path parameter for redirect handler exepted"),
                            status_code.clone(),
                        ))
                    }
                };

                routes.insert(r.path.clone(), handler);
            }
            vhosts.insert(
                vh.domain.clone(),
                VirtualHostPlan {
                    domain: vh.domain.clone(),
                    routes: routes,
                },
            );
        }

        ServerPlan {
            virtual_hosts: vhosts,
        }
    }
}
