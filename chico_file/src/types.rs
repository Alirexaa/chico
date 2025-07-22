use url::Url;

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
    Proxy(LoadBalancer),
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
pub enum LoadBalancer {
    NoBalancer(Upstream),
    RoundRobin(Vec<Upstream>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Upstream {
    url: url::Url,
    host_addrs: String,
}

impl Upstream {
    pub fn new(upstream_addr: String) -> Result<Self, String> {
        let parse_result = Url::parse(&upstream_addr);
        let Ok(url) = parse_result else {
            return Err(parse_result.err().unwrap().to_string());
        };

        let host = url.host();

        let Some(host) = host else {
            return Err("host name is not valid".to_string());
        };

        let port = url.port().map_or(80, |port| port);

        let host_and_port = format!("{host}:{port}");

        Ok(Upstream {
            host_addrs: host_and_port,
            url,
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

    use crate::types::Upstream;

    use super::Handler;

    #[test]
    fn test_handler_type_name() {
        let handler = Handler::File(String::new());
        assert_eq!(handler.type_name(), "File");

        let handler = Handler::Proxy(crate::types::LoadBalancer::NoBalancer(
            Upstream::new("http://127.0.0.1".to_string()).unwrap(),
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
}
