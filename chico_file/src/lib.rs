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
    println!("input: {:?}", input);
    let (input, _) = tag("#")(input)?;
    println!("tag: {:?}", input);

    let (input, _) = opt(not_line_ending)(input)?;
    let (input, _) = line_ending(input)?;
    println!("input2: {:?}", input);

    Ok((input, ()))
}

// Parses a domain like "example.com { ... }"
fn parse_virtual_host(input: &str) -> IResult<&str, VirtualHost> {
    let (input, _) = multispace0(input)?;
    let (input, domain) = take_while1(|c: char| !c.is_whitespace() && c != '{')(input)?;
    let (input, _) = multispace0(input)?;

    let (input, routes) = delimited(char('{'), many0(parse_route), char('}'))(input)?;
    Ok((
        input,
        VirtualHost {
            domain: domain.to_string(),
            routes,
        },
    ))
}

// Parses a route like "route /path { ... }"
fn parse_route(input: &str) -> IResult<&str, Route> {
    let (input, _) = multispace0(input)?;

    let (input, _) = tag("route")(input)?;

    let (input, _) = space1(input)?;
    let (input, _) = multispace0(input)?;
    let (input, path) = take_while1(|c: char| !c.is_whitespace() && c != '{')(input)?;

    let (input, _) = multispace0(input)?;

    let (input, (handler, middlewares)) =
        delimited(char('{'), parse_route_contents, char('}'))(input)?;
    let (input, _) = multispace0(input)?;
    Ok((
        input,
        Route {
            path: path.to_string(),
            handler,
            middlewares,
        },
    ))
}

// Parses handler + middleware settings inside a route block
fn parse_route_contents(input: &str) -> IResult<&str, (Handler, Vec<Middleware>)> {
    let (input, _) = multispace0(input)?;
    let (input, handler) = parse_handler(input)?;
    let (input, _) = multispace0(input)?;

    let (input, middlewares) = many0(parse_middleware)(input)?;
    let (input, _) = multispace0(input)?;

    Ok((input, (handler, middlewares)))
}

// Parses different handlers (file, proxy, dir, browse)
fn parse_handler(input: &str) -> IResult<&str, Handler> {
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
        parse_virtual_host,
        map(parse_comment, |_| VirtualHost {
            domain: "".to_string(),
            routes: vec![],
        }),
    )))(input)
    .map(|(i, hosts)| {
        (
            i,
            hosts.into_iter().filter(|h| !h.domain.is_empty()).collect(),
        )
    })
}
