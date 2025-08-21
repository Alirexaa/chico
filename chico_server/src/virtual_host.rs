use std::str::FromStr;

use chico_file::types::VirtualHost;
use crates_uri::UriExt;
use http::Uri;

pub trait VirtualHostExt {
    fn get_port(&self) -> u16;
}

impl VirtualHostExt for VirtualHost {
    fn get_port(&self) -> u16 {
        Uri::from_str(&self.domain)
            .expect("Expected Valid host")
            .get_port()
    }
}
