#![cfg_attr(feature = "strict", deny(warnings))]

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{
        char, digit1, line_ending, multispace0, none_of, not_line_ending, space1,
    },
    combinator::{map, opt},
    multi::many0,
    sequence::{delimited, preceded, tuple},
    IResult,
};

#[derive(Debug)]
pub struct VirtualHost {
    pub domain: String,
    pub routes: Vec<Route>,
}

#[derive(Debug)]
pub struct Route {
    pub path: String,
    pub handler: Handler,
    pub middlewares: Vec<Middleware>,
}

#[derive(Debug)]
pub enum Handler {
    File(String),
    Proxy(String),
    Dir(String),
    Browse(String),
    Respond {
        status: Option<u16>,
        body: Option<String>,
    },
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum HeaderOperator {
    /// Prefix with + to add the field instead of overwriting (setting) the field if it already exists; header fields can appear more than once in a request.
    Add,
    /// No prefix means the field is set if it doesn't exist, and otherwise it is replaced.
    Set,
    /// Prefix with > to set the field, and enable defer, as a shortcut.
    DeferSet,
    /// Prefix with - to delete the field. The field may use prefix or suffix * wildcards to delete all matching fields.
    Delete,
    /// <replace> is the replacement value; required if performing a search-and-replace. Use $1 or $2 and so on to reference capture groups from the search pattern. If the replacement value is "", then the matching text is removed from the value.
    Replace,
    /// Replace with defer behavior
    DeferReplace,
    /// Prefix with ? to set a default value for the field. The field is only written if it doesn't yet exist.
    Default,
}

// Parses a single-line comment like "# this is a comment"
fn parse_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("#")(input)?;
    let (input, _) = opt(not_line_ending)(input)?;
    let (input, _) = opt(line_ending)(input)?;
    Ok((input, ()))
}

// Parses a domain like "example.com { ... }"
fn parse_virtual_host(input: &str) -> IResult<&str, VirtualHost> {
    let (input, _) = multispace0(input)?;
    let (input, domain) = take_while1(|c: char| !c.is_whitespace() && c != '{')(input)?;
    let (input, _) = multispace0(input)?;

    let (input, routes) = delimited(
        char('{'),
        many0(alt((
            map(parse_route, Some),       // Parses routes as Some(Route)
            map(parse_comment, |_| None), // Ignores comments, returning None
        ))),
        char('}'),
    )(input)?;

    // Allow comments before virtual host ending
    let (input, _) = many0(parse_comment)(input)?;

    // Use filter_map to remove None values and unwrap Some(Route)
    let routes: Vec<Route> = routes.into_iter().flatten().flatten().collect();

    Ok((
        input,
        VirtualHost {
            domain: domain.to_string(),
            routes,
        },
    ))
}

// Parses a route like "route /path { ... }"
fn parse_route(input: &str) -> IResult<&str, Option<Route>> {
    let (input, _) = multispace0(input)?;

    // Allow comments before a route
    let (input, _) = many0(parse_comment)(input)?;

    let (input, _) = tag("route")(input)?;
    let (input, _) = space1(input)?;
    let (input, path) = take_while1(|c: char| !c.is_whitespace() && c != '{')(input)?;
    let (input, _) = multispace0(input)?;

    let (input, (handler, middlewares)) =
        delimited(char('{'), parse_route_contents, char('}'))(input)?;

    let (input, _) = multispace0(input)?;

    // Allow comments after a route
    let (input, _) = many0(parse_comment)(input)?;

    let (input, _) = multispace0(input)?;

    Ok((
        input,
        Some(Route {
            path: path.to_string(),
            handler,
            middlewares,
        }),
    ))
}

// Parses handler + middleware settings inside a route block
fn parse_route_contents(input: &str) -> IResult<&str, (Handler, Vec<Middleware>)> {
    let (input, _) = multispace0(input)?;

    // Allow comments before handler
    let (input, _) = many0(parse_comment)(input)?;

    let (input, handler) = parse_handler(input)?;
    let (input, _) = multispace0(input)?;

    // Allow comments before middlewares
    let (input, middlewares) = many0(alt((
        map(parse_comment, |_| None), // Allow comments inside route block
        map(parse_middleware, Some),  // Parse middleware
    )))(input)?;

    let (input, _) = multispace0(input)?;

    // Remove None values (from comments)
    let middlewares = middlewares.into_iter().flatten().collect();

    Ok((input, (handler, middlewares)))
}

// Parses different handlers (file, proxy, dir, browse)
fn parse_handler(input: &str) -> IResult<&str, Handler> {
    let (input, _) = multispace0(input)?;
    alt((
        map(preceded(tag("file"), parse_value), Handler::File),
        map(preceded(tag("proxy"), parse_value), Handler::Proxy),
        map(preceded(tag("dir"), parse_value), Handler::Dir),
        map(preceded(tag("browse"), parse_value), Handler::Browse),
        map(
            preceded(tag("respond"), parse_respond_handler),
            |(status, body)| Handler::Respond {
                status: status,
                body: body,
            },
        ),
    ))(input)
}

// Parses middleware options like "gzip", "cors", "log", "rate_limit 10", "auth admin pass"
fn parse_middleware(input: &str) -> IResult<&str, Middleware> {
    let (input, _) = multispace0(input)?;

    alt((
        map(tag("gzip"), |_| Middleware::Gzip),
        map(tag("cors"), |_| Middleware::Cors),
        map(tag("log"), |_| Middleware::Log),
        parse_rate_limit,
        parse_auth,
        parse_cache,
    ))(input)
}

// Parses "rate_limit <N>"
fn parse_rate_limit(input: &str) -> IResult<&str, Middleware> {
    let (input, _) = tag("rate_limit")(input)?;
    let (input, _) = space1(input)?;
    let (input, num) = take_while1(|c: char| c.is_digit(10))(input)?;
    Ok((input, Middleware::RateLimit(num.parse().unwrap())))
}

// Parses "auth <username> <password>"
fn parse_auth(input: &str) -> IResult<&str, Middleware> {
    let (input, _) = tag("auth")(input)?;
    let (input, _) = space1(input)?;
    let (input, username) = take_while1(|c: char| !c.is_whitespace())(input)?;
    let (input, _) = space1(input)?;
    let (input, password) = take_while1(|c: char| !c.is_whitespace())(input)?;
    Ok((
        input,
        Middleware::Auth {
            username: username.to_string(),
            password: password.to_string(),
        },
    ))
}

// Parses "cache <duration>"
fn parse_cache(input: &str) -> IResult<&str, Middleware> {
    let (input, _) = tag("cache")(input)?;
    let (input, _) = space1(input)?;
    let (input, duration) = take_while1(|c: char| !c.is_whitespace())(input)?;
    Ok((input, Middleware::Cache(duration.to_string())))
}

// Parses values like "index.html" or "http://localhost:3000"
fn parse_value(input: &str) -> IResult<&str, String> {
    let (input, _) = space1(input)?;
    let (input, value) = take_while1(|c: char| !c.is_whitespace())(input)?;
    Ok((input, value.to_string()))
}

// Parses values like "index.html" or "http://localhost:3000"
fn parse_respond_handler(input: &str) -> IResult<&str, (Option<u16>, Option<String>)> {
    let (input, _) = space1(input)?;

    let (input, result) = alt((
        map(parse_literal_u16, |(body, status)| {
            (Some(body), Some(status))
        }),
        map(string_literal, |body| (Some(body), None)),
        map(parse_u16, |status| (None, Some(status))),
    ))(input)?;

    Ok((input, (result.1, result.0)))
}

// Parses the entire configuration, allowing comments and empty lines
pub fn parse_config(input: &str) -> IResult<&str, Vec<VirtualHost>> {
    many0(alt((
        map(parse_virtual_host, Some),
        map(parse_comment, |_| None), // Skip comments
    )))(input)
    .map(|(i, hosts)| (i, hosts.into_iter().flatten().collect()))
}

#[allow(dead_code)]
/// Parses a string literal  
fn string_literal(input: &str) -> IResult<&str, String> {
    delimited(
        char('"'),
        map(many0(none_of("\"")), |chars: Vec<char>| {
            chars.into_iter().collect()
        }),
        char('"'),
    )(input)
}

/// Parses an unsigned 16-bit integer (u16)  
fn parse_u16(input: &str) -> IResult<&str, u16> {
    // We use digit1 to ensure we have at least one digit
    let (input, _) = multispace0(input)?;
    let (input, digits) = digit1(input)?;

    // Convert the digits string to a u16
    // This will return an error if the value is too large for u16
    let value = digits
        .parse::<u16>()
        .map_err(|_| nom::Err::Error((input, nom::error::ErrorKind::Digit)))
        .unwrap();

    Ok((input, value))
}

/// Parses a string literal and an unsigned 16-bit integer (u16) example: "Some String" 123
fn parse_literal_u16(input: &str) -> IResult<&str, (String, u16)> {
    tuple((string_literal, preceded(space1, parse_u16)))(input)
}
