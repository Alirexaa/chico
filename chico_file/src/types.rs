#[derive(Debug, PartialEq)]
pub struct Config {
    pub virtual_hosts: Vec<VirtualHost>,
}

#[derive(Debug, PartialEq)]
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
    Proxy(String),
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
