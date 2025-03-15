use http::{uri::Scheme, Uri};

pub trait UriExt {
    #[allow(dead_code)]
    fn get_port(&self) -> u16;
}

impl UriExt for Uri {
    fn get_port(&self) -> u16 {
        {
            let uri = self;
            let port = uri.port_u16();
            let scheme = uri.scheme();
            port.unwrap_or_else(|| {
                if scheme == Some(&Scheme::HTTPS) {
                    443
                } else {
                    80
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use http::Uri;
    use rstest::rstest;

    use super::UriExt;

    #[rstest]
    #[case("localhost", 80)]
    #[case("http://localhost", 80)]
    #[case("http://localhost:3000", 3000)]
    #[case("https://example.com", 443)]
    #[case("https://example.com:3000", 3000)]
    #[case("example.com:3000", 3000)]
    #[case("example.com", 80)]
    fn test_get_port(#[case] uri: &str, #[case] port: u16) {
        let uri = Uri::from_str(uri).unwrap();
        assert_eq!(uri.get_port(), port);
    }
}
