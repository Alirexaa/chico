use std::{
    net::{AddrParseError, SocketAddr},
    str::FromStr,
};

#[allow(dead_code)]
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Node {
    pub addr: SocketAddr,
}

#[allow(dead_code)]
impl Node {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

impl From<SocketAddr> for Node {
    fn from(value: SocketAddr) -> Self {
        Node::new(value)
    }
}

impl FromStr for Node {
    type Err = AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<SocketAddr>().and_then(|addr| Ok(addr.into()))
    }
}
