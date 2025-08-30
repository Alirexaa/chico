use crates_uri::UriExt;

#[derive(Debug, PartialEq, Clone)]
pub struct Config {
    pub virtual_hosts: Vec<VirtualHost>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VirtualHost {
    pub domain: String,
    pub routes: Vec<Route>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Route {
    pub path: String,
    pub handler: Handler,
    pub middlewares: Vec<Middleware>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Handler {
    File(String),
    Proxy(ProxyConfig),
    Dir(String),
    Browse(String),
    Respond {
        status: Option<u16>,
        body: Option<String>,
    },
    Redirect {
        path: Option<String>,
        status_code: Option<u16>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct ProxyConfig {
    pub load_balancer: LoadBalancer,
    pub request_timeout: Option<u64>,    // in seconds
    pub connection_timeout: Option<u64>, // in seconds
}

impl ProxyConfig {
    pub fn new(load_balancer: LoadBalancer) -> Self {
        Self {
            load_balancer,
            request_timeout: None,
            connection_timeout: None,
        }
    }
    
    pub fn with_timeouts(load_balancer: LoadBalancer, request_timeout: Option<u64>, connection_timeout: Option<u64>) -> Self {
        Self {
            load_balancer,
            request_timeout,
            connection_timeout,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum LoadBalancer {
    NoBalancer(Upstream),
    RoundRobin(Vec<Upstream>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Upstream {
    uri: http::Uri,
    host_addrs: String,
}

impl Upstream {
    pub fn new(upstream_addr: String) -> Result<Self, String> {
        let parse_result: Result<http::Uri, http::uri::InvalidUri> = upstream_addr.parse();
        let Ok(uri) = parse_result else {
            return Err(parse_result.err().unwrap().to_string());
        };

        let host = uri.host();

        let Some(host) = host else {
            return Err("host name is not valid".to_string());
        };

        let port = &uri.get_port();

        let host_and_port = format!("{host}:{port}");

        Ok(Upstream {
            host_addrs: host_and_port,
            uri,
        })
    }

    pub fn get_host_port(&self) -> &str {
        &self.host_addrs
    }
}

impl Handler {
    pub fn type_name(&self) -> &str {
        match self {
            Handler::File(_) => "File",
            Handler::Proxy(_) => "Proxy",
            Handler::Dir(_) => "Dir",
            Handler::Browse(_) => "Browse",
            Handler::Respond { status: _, body: _ } => "Respond",
            Handler::Redirect {
                path: _,
                status_code: _,
            } => "Redirect",
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Middleware {
    Gzip,
    Cors,
    Log,
    RateLimit(u32),
    Auth {
        username: String,
        password: String,
    },
    Cache(String),
    /// First Parameter is the header name with prefix operator, second is the header value, third is for replace value
    Header {
        operator: HeaderOperator,
        name: String,
        value: Option<String>,
        replace_with: Option<String>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum HeaderOperator {
    /// Prefix with + to add the field instead of overwriting (setting) the field if it already exists; header fields can appear more than once in a request.
    Add,
    /// Prefix with = means the field is set if it doesn't exist, and otherwise it is replaced.
    Set,
    /// Prefix with > to set the field, and enable defer, as a shortcut.
    DeferSet,
    /// Prefix with - to delete the field. The field may use prefix or suffix * wildcards to delete all matching fields.
    Delete,
    /// Prefix with ~ <replace> is the replacement value; required if performing a search-and-replace. Use $1 or $2 and so on to reference capture groups from the search pattern. If the replacement value is "", then the matching text is removed from the value.
    Replace,
    /// Prefix with ~> with defer behavior
    DeferReplace,
    /// Prefix with ? to set a default value for the field. The field is only written if it doesn't yet exist.
    Default,
}

#[cfg(test)]
mod tests {

    use rstest::rstest;

    use crate::types::Upstream;

    use super::Handler;

    #[test]
    fn test_handler_type_name() {
        let handler = Handler::File(String::new());
        assert_eq!(handler.type_name(), "File");

        let handler = Handler::Proxy(crate::types::ProxyConfig::new(
            crate::types::LoadBalancer::NoBalancer(
                Upstream::new("http://127.0.0.1".to_string()).unwrap(),
            )
        ));
        assert_eq!(handler.type_name(), "Proxy");

        let handler = Handler::Dir(String::new());
        assert_eq!(handler.type_name(), "Dir");

        let handler = Handler::Browse(String::new());
        assert_eq!(handler.type_name(), "Browse");

        let handler = Handler::Respond {
            status: None,
            body: None,
        };
        assert_eq!(handler.type_name(), "Respond");

        let handler = Handler::Redirect {
            path: None,
            status_code: None,
        };
        assert_eq!(handler.type_name(), "Redirect");
    }

    #[rstest]
    #[case("localhost", "localhost:80")]
    #[case("http://localhost", "localhost:80")]
    #[case("localhost:3000", "localhost:3000")]
    #[case("http://localhost:3000", "localhost:3000")]
    #[case("https://localhost", "localhost:443")]
    #[case("https://localhost:8443", "localhost:8443")]
    #[case("example.com", "example.com:80")]
    #[case("http://example.com", "example.com:80")]
    #[case("http://example.com:3000", "example.com:3000")]
    #[case("https://example.com", "example.com:443")]
    #[case("https://example.com:8443", "example.com:8443")]
    fn test_upstream_new_ok(#[case] given_addrs: &str, #[case] host_and_port: &str) {
        let upstream = Upstream::new(given_addrs.to_string());
        let upstream = claims::assert_ok!(upstream);
        assert_eq!(upstream.get_host_port(), host_and_port)
    }

    #[rstest]
    #[case("")]
    #[case("/addrs")]
    fn test_upstream_new_err(#[case] given_addrs: &str) {
        let upstream = Upstream::new(given_addrs.to_string());
        claims::assert_err!(upstream);
    }
}
