use http::{uri::Scheme, Uri};

/// Extension trait for `Uri` to provide additional functionality.
pub trait UriExt {
    /// Retrieves the port number from the `Uri`.
    ///
    /// If the `Uri` does not explicitly specify a port, this method returns the default port
    /// for the scheme: 443 for HTTPS and 80 for HTTP.
    ///
    /// # Returns
    ///
    /// * `u16` - The port number.
    ///
    /// # Examples
    ///
    /// ```
    /// use crates_uri::UriExt;
    /// use http::Uri;
    ///
    /// let uri: Uri = "https://example.com".parse().unwrap();
    /// assert_eq!(uri.get_port(), 443);
    ///
    /// let uri: Uri = "http://example.com".parse().unwrap();
    /// assert_eq!(uri.get_port(), 80);
    ///
    /// let uri: Uri = "http://example.com:8080".parse().unwrap();
    /// assert_eq!(uri.get_port(), 8080);
    /// ```
    #[allow(dead_code)]
    fn get_port(&self) -> u16;
}

impl UriExt for Uri {
    fn get_port(&self) -> u16 {
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
