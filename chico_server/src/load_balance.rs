pub mod node;
pub mod round_robin;

use std::sync::Arc;

use chico_file::types::Config;

use crate::load_balance::{node::Node, round_robin::RoundRobinBalancer};

#[allow(dead_code)]
pub struct LoadBalancerProvider {
    config: Arc<Config>,
}

#[allow(dead_code)]
impl LoadBalancerProvider {
    pub fn new(config: Arc<Config>) -> LoadBalancerProvider {
        LoadBalancerProvider { config }
    }
    pub fn get_balancer(&self, _req_url: String) -> Box<dyn LoadBalance> {
        Box::new(RoundRobinBalancer::new(vec![]))
    }
}

pub trait LoadBalance: Send + Sync {
    fn get_node(&self) -> Option<Arc<Node>>;
}
