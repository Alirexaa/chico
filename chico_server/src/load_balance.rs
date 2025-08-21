use std::sync::Arc;

use crate::load_balance::node::Node;

pub mod node;
pub mod round_robin;

pub trait LoadBalance: Send + Sync {
    fn get_node(&self) -> Option<Arc<Node>>;
}

pub struct SingleUpstream {
    node: Arc<Node>,
}

impl SingleUpstream {
    pub fn new(node: Node) -> Self {
        Self {
            node: Arc::new(node),
        }
    }
}

impl LoadBalance for SingleUpstream {
    fn get_node(&self) -> Option<Arc<Node>> {
        Some(self.node.clone())
    }
}
