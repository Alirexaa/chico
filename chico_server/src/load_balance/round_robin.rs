//! # RoundRobinBalancer
//!
//! A concurrent round-robin load balancer that distributes requests evenly across a list of upstream `Node`s.
//!
//! - Thread-safe via atomic counter.
//! - Automatically resets counter when it exceeds a configured threshold to prevent overflow.
//! - Uses `Arc<Node>` for efficient sharing.
//!
//! ## Example
//! ```rust
//! let nodes = vec![
//!     "127.0.0.1:80".parse().unwrap(),
//!     "1.0.0.1:9090".parse().unwrap(),
//! ];
//! let balancer = RoundRobinBalancer::new(nodes);
//! let node = balancer.get_node();
//! ```

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crate::load_balance::{node::Node, LoadBalance};

/// A thread-safe round-robin load balancer.
///
/// This balancer distributes requests across a fixed list of upstream nodes
/// by rotating through them using an atomic counter.
///
/// If the counter exceeds a configured `RESET_THRESHOLD`, it is reset
/// to avoid integer overflow.
pub struct RoundRobinBalancer {
    nodes: Arc<[Arc<Node>]>,
    counter: AtomicUsize,
}

#[cfg(test)]
const RESET_THRESHOLD: usize = 10000;

#[cfg(not(test))]
const RESET_THRESHOLD: usize = usize::MAX / 2;

#[allow(dead_code)]
impl RoundRobinBalancer {
    /// Creates a new `RoundRobinBalancer` from a list of nodes.
    ///
    /// Each node is internally wrapped in an `Arc` for cheap cloning and sharing.
    pub fn new(nodes: Vec<Node>) -> Self {
        let arc_nodes: Vec<Arc<Node>> = nodes.into_iter().map(Arc::new).collect();

        Self {
            nodes: arc_nodes.into(),
            counter: AtomicUsize::new(0),
        }
    }

    /// Returns the next node to use, rotating through the list.
    ///
    /// If the node list is empty, returns `None`.
    ///
    /// This method is safe to call from multiple threads concurrently.
    fn next(&self) -> Option<Arc<Node>> {
        let len = self.nodes.len();
        if len == 0 {
            return None;
        }

        let current = self.counter.fetch_add(1, Ordering::Relaxed);
        let index = current % len;

        if current >= RESET_THRESHOLD {
            let _ = self.counter.compare_exchange(
                current + 1,
                index + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            );
        }

        Some(self.nodes[index].clone())
    }
}

impl LoadBalance for RoundRobinBalancer {
    fn get_node(&self) -> Option<Arc<Node>> {
        self.next()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use claims::assert_some_eq;

    use super::*;

    #[test]
    fn test_round_robin_balancer() {
        let nodes: Vec<Node> = vec![
            "127.0.0.1:80".parse().unwrap(),
            "1.0.0.1:9090".parse().unwrap(),
            "1.0.0.2:8080".parse().unwrap(),
        ];
        let balancer = RoundRobinBalancer::new(nodes);

        for _ in 0..=RESET_THRESHOLD + 1 {
            assert_some_eq!(
                balancer.get_node(),
                Arc::new("127.0.0.1:80".parse().unwrap())
            );
            assert_some_eq!(
                balancer.get_node(),
                Arc::new("1.0.0.1:9090".parse().unwrap())
            );
            assert_some_eq!(
                balancer.get_node(),
                Arc::new("1.0.0.2:8080".parse().unwrap())
            );
        }
    }

    #[test]
    fn test_empty_nodes() {
        let balancer = RoundRobinBalancer::new(vec![]);
        assert!(balancer.get_node().is_none());
    }

    #[test]
    fn test_single_node() {
        let node: Node = "127.0.0.1:80".parse().unwrap();
        let balancer = RoundRobinBalancer::new(vec![node]);
        for _ in 0..100 {
            assert_eq!(
                balancer.get_node().unwrap(),
                Arc::new("127.0.0.1:80".parse().unwrap())
            );
        }
    }
    #[test]
    fn test_counter_reset() {
        let nodes: Vec<Node> = vec![
            "127.0.0.1:80".parse().unwrap(),
            "1.0.0.1:9090".parse().unwrap(),
            "1.0.0.2:8080".parse().unwrap(),
        ];
        let balancer = RoundRobinBalancer::new(nodes);

        // Artificially set the counter close to the threshold
        balancer
            .counter
            .store(RESET_THRESHOLD - 1, Ordering::Relaxed);

        // Trigger get_node() enough to cross threshold and reset
        for _ in 0..5 {
            let _ = balancer.get_node();
        }

        let val_after_reset = balancer.counter.load(Ordering::Relaxed);
        assert!(
            val_after_reset < RESET_THRESHOLD,
            "Counter did not reset properly: {}",
            val_after_reset
        );
    }

    /// Stress test: Ensures thread-safe access and even load distribution across many threads.
    #[test]
    fn test_concurrent_access() {
        let nodes: Vec<Node> = vec![
            "127.0.0.1:80".parse().unwrap(),
            "1.0.0.1:9090".parse().unwrap(),
            "1.0.0.2:8080".parse().unwrap(),
        ];
        let balancer = Arc::new(RoundRobinBalancer::new(nodes.clone()));
        let result_counts: Arc<Mutex<Vec<(Node, usize)>>> = Arc::new(Mutex::new(Vec::new()));

        let num_threads = 30;
        let iterations_per_thread = 1000;

        let mut handles = vec![];

        for _ in 0..num_threads {
            let balancer = Arc::clone(&balancer);
            let result_counts = Arc::clone(&result_counts);

            handles.push(std::thread::spawn(move || {
                for _ in 0..iterations_per_thread {
                    if let Some(node) = balancer.get_node() {
                        let mut counts = result_counts.lock().unwrap();

                        if let Some((_, count)) = counts.iter_mut().find(|(n, _)| n == &*node) {
                            *count += 1;
                        } else {
                            counts.push(((*node).clone(), 1));
                        }
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let counts = result_counts.lock().unwrap();
        let total: usize = counts.iter().map(|(_, count)| *count).sum();

        assert_eq!(total, num_threads * iterations_per_thread);

        let expected = total / nodes.len();
        for (node, count) in counts.iter() {
            let deviation = ((count - expected) as f64 / expected as f64).abs();
            assert!(
                deviation < 0.0005,
                "Node {:?} got {} requests, expected ~{}, too much deviation. deviation {}",
                node,
                count,
                expected,
                deviation
            );
        }
    }
}
