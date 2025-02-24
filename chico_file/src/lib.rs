#![cfg_attr(feature = "strict", deny(warnings))]
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, line_ending, multispace0, not_line_ending, space1},
    combinator::{map, opt},
    multi::many0,
    sequence::{delimited, preceded},
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
}

#[derive(Debug)]
pub enum Middleware {
    Gzip,
    Cors,
    Log,
    RateLimit(u32),
    Auth { username: String, password: String },
    Cache(String),
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

// Parses the entire configuration, allowing comments and empty lines
pub fn parse_config(input: &str) -> IResult<&str, Vec<VirtualHost>> {
    many0(alt((
        map(parse_virtual_host, Some),
        map(parse_comment, |_| None), // Skip comments
    )))(input)
    .map(|(i, hosts)| (i, hosts.into_iter().flatten().collect()))
}
