#![cfg_attr(feature = "strict", deny(warnings))]

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{
        char, digit1, multispace0, multispace1, none_of, not_line_ending, space1,
    },
    combinator::{map, opt},
    error::{Error, ErrorKind},
    multi::{many0, many1},
    sequence::{delimited, preceded, tuple},
    Err, IResult,
};
use types::{Config, VirtualHost};

use crate::types::Upstream;

pub mod types;

// Parses a single-line comment like "# this is a comment"
fn parse_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("#")(input)?;
    let (input, _) = opt(not_line_ending)(input)?;
    Ok((input, ()))
}

// Parses a domain like "example.com { ... }"
fn parse_virtual_host(input: &str) -> IResult<&str, types::VirtualHost> {
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
    let routes: Vec<types::Route> = routes.into_iter().flatten().flatten().collect();

    Ok((
        input,
        types::VirtualHost {
            domain: domain.to_string(),
            routes,
        },
    ))
}

// Parses a route like "route /path { ... }"
fn parse_route(input: &str) -> IResult<&str, Option<types::Route>> {
    let (input, _) = multispace0(input)?;

    // Allow comments before a route
    let (input, _) = many0(parse_comment)(input)?;
    let (input, _) = multispace0(input)?;

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
        Some(types::Route {
            path: path.to_string(),
            handler,
            middlewares,
        }),
    ))
}

// Parses handler + middleware settings inside a route block
fn parse_route_contents(input: &str) -> IResult<&str, (types::Handler, Vec<types::Middleware>)> {
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
fn parse_handler(input: &str) -> IResult<&str, types::Handler> {
    let (input, _) = multispace0(input)?;
    alt((
        map(preceded(tag("file"), parse_value), types::Handler::File),
        parse_proxy_handler,
        map(preceded(tag("dir"), parse_value), types::Handler::Dir),
        map(preceded(tag("browse"), parse_value), types::Handler::Browse),
        map(
            preceded(tag("respond"), parse_respond_handler_args),
            |(status, body)| types::Handler::Respond { status, body },
        ),
        map(
            preceded(tag("redirect"), parse_redirect_handler_args),
            |(status_code, path)| types::Handler::Redirect { status_code, path },
        ),
    ))(input)
}

// Parses proxy handlers - supports both old and new syntax
fn parse_proxy_handler(input: &str) -> IResult<&str, types::Handler> {
    let (input, _) = preceded(tag("proxy"), multispace0)(input)?;

    alt((
        // New block syntax: proxy { upstreams ...; lb_policy ... }
        parse_proxy_block,
        // Old simple syntax: proxy http://localhost:3000 (for backward compatibility)
        map(take_while1(|c: char| !c.is_whitespace()), |addr: &str| {
            types::Handler::Proxy(types::LoadBalancer::NoBalancer(
                Upstream::new(addr.to_string()).unwrap(),
            ))
        }),
    ))(input)
}

// Parses the new proxy block format
fn parse_proxy_block(input: &str) -> IResult<&str, types::Handler> {
    let (input, (upstreams, lb_policy)) =
        delimited(char('{'), parse_proxy_block_contents, char('}'))(input)?;

    let load_balancer = match lb_policy.as_deref() {
        Some("round_robin") => {
            if upstreams.len() == 1 {
                // Single upstream with round_robin policy still uses NoBalancer
                types::LoadBalancer::NoBalancer(upstreams.into_iter().next().unwrap())
            } else {
                types::LoadBalancer::RoundRobin(upstreams)
            }
        }
        None | Some("") => {
            // Default: no load balancer specified or empty value
            if upstreams.len() == 1 {
                types::LoadBalancer::NoBalancer(upstreams.into_iter().next().unwrap())
            } else {
                // Multiple upstreams without lb_policy defaults to round_robin
                types::LoadBalancer::RoundRobin(upstreams)
            }
        }
        Some(_policy) => {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                ErrorKind::Alt,
            )));
        }
    };

    Ok((input, types::Handler::Proxy(load_balancer)))
}

// Parses the contents inside the proxy block
fn parse_proxy_block_contents(input: &str) -> IResult<&str, (Vec<Upstream>, Option<String>)> {
    let (input, _) = multispace0(input)?;

    // Allow comments before upstreams
    let (input, _) = many0(parse_comment)(input)?;
    let (input, _) = multispace0(input)?;

    // Parse upstreams line
    let (input, _) = tag("upstreams")(input)?;
    let (input, _) = multispace1(input)?;

    // Parse upstream addresses until we hit a newline or lb_policy keyword
    let (input, upstreams) = parse_upstream_addresses(input)?;
    let (input, _) = multispace0(input)?;

    // Allow comments before lb_policy
    let (input, _) = many0(parse_comment)(input)?;
    let (input, _) = multispace0(input)?;

    // Check for lb_policy
    let (input, lb_policy) = opt(tuple((
        tag("lb_policy"),
        opt(preceded(
            multispace1,
            take_while1(|c: char| !c.is_whitespace() && c != '}'),
        )),
    )))(input)?;

    let (input, _) = multispace0(input)?;

    // Allow comments after lb_policy
    let (input, _) = many0(parse_comment)(input)?;
    let (input, _) = multispace0(input)?;

    let policy_str = lb_policy.and_then(|(_, policy_opt)| policy_opt.map(|s| s.to_string()));

    Ok((input, (upstreams, policy_str)))
}

// Parse upstream addresses one by one until we hit lb_policy or end
fn parse_upstream_addresses(input: &str) -> IResult<&str, Vec<Upstream>> {
    let mut upstreams = Vec::new();
    let mut remaining = input;

    loop {
        // Skip whitespace and comments
        let (next_input, _) = multispace0(remaining)?;
        let (next_input, _) = many0(parse_comment)(next_input)?;
        let (next_input, _) = multispace0(next_input)?;
        remaining = next_input;

        // Check if we've hit lb_policy or } or end
        if remaining.starts_with("lb_policy") || remaining.starts_with("}") || remaining.is_empty()
        {
            break;
        }

        // Parse the next upstream address
        let (next_input, addr) = take_while1(|c: char| !c.is_whitespace())(remaining)?;

        // Make sure it's not lb_policy
        if addr == "lb_policy" {
            break;
        }

        // Convert to Upstream
        match Upstream::new(addr.to_string()) {
            Ok(upstream) => upstreams.push(upstream),
            Err(_) => {
                return Err(nom::Err::Error(nom::error::Error::new(
                    remaining,
                    ErrorKind::Alt,
                )));
            }
        }

        remaining = next_input;
    }

    if upstreams.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            ErrorKind::Alt,
        )));
    }

    Ok((remaining, upstreams))
}

// Parses middleware options like "gzip", "cors", "log", "rate_limit 10", "auth admin pass"
fn parse_middleware(input: &str) -> IResult<&str, types::Middleware> {
    let (input, _) = multispace0(input)?;

    alt((
        map(tag("gzip"), |_| types::Middleware::Gzip),
        map(tag("cors"), |_| types::Middleware::Cors),
        map(tag("log"), |_| types::Middleware::Log),
        parse_rate_limit,
        parse_auth,
        parse_cache,
        parse_header,
    ))(input)
}

// Parses "rate_limit <N>"
fn parse_rate_limit(input: &str) -> IResult<&str, types::Middleware> {
    let (input, _) = tag("rate_limit")(input)?;
    let (input, _) = space1(input)?;
    let (input, num) = take_while1(|c: char| c.is_ascii_digit())(input)?;
    Ok((input, types::Middleware::RateLimit(num.parse().unwrap())))
}

// Parses "auth <username> <password>"
fn parse_auth(input: &str) -> IResult<&str, types::Middleware> {
    let (input, _) = tag("auth")(input)?;
    let (input, _) = space1(input)?;
    let (input, username) = take_while1(|c: char| !c.is_whitespace())(input)?;
    let (input, _) = space1(input)?;
    let (input, password) = take_while1(|c: char| !c.is_whitespace())(input)?;
    Ok((
        input,
        types::Middleware::Auth {
            username: username.to_string(),
            password: password.to_string(),
        },
    ))
}

// Parses "cache <duration>"
fn parse_cache(input: &str) -> IResult<&str, types::Middleware> {
    let (input, _) = tag("cache")(input)?;
    let (input, _) = space1(input)?;
    let (input, duration) = take_while1(|c: char| !c.is_whitespace())(input)?;
    Ok((input, types::Middleware::Cache(duration.to_string())))
}

// Parses "header <key> <value>" or "header <key> <value> <replace_with>" or "header <key>"
fn parse_header(input: &str) -> IResult<&str, types::Middleware> {
    let (input, _) = tag("header")(input)?;
    let (input, _) = space1(input)?;

    // Parse the header operator
    let (input, operator) = alt((
        // two operator characters should be parsed first
        map(tag("~>"), |_| types::HeaderOperator::DeferReplace),
        map(tag("+"), |_| types::HeaderOperator::Add),
        map(tag(">"), |_| types::HeaderOperator::DeferSet),
        map(tag("-"), |_| types::HeaderOperator::Delete),
        map(tag("?"), |_| types::HeaderOperator::Default),
        map(tag("="), |_| types::HeaderOperator::Set),
        map(tag("~"), |_| types::HeaderOperator::Replace),
    ))(input)?;

    // Parse the header name and value and replace_with if present
    let (input, (name, value, replace_with)) = tuple((
        take_while1(|c: char| !c.is_whitespace()),
        opt(preceded(space1, take_while1(|c: char| !c.is_whitespace()))),
        opt(preceded(space1, take_while1(|c: char| !c.is_whitespace()))),
    ))(input)?;

    Ok((
        input,
        types::Middleware::Header {
            operator,
            name: name.to_string(),
            value: value.map(|s| s.to_string()),
            replace_with: replace_with.map(|s| s.to_string()),
        },
    ))
}

// Parses values like "index.html" or "http://localhost:3000"
fn parse_value(input: &str) -> IResult<&str, String> {
    let (input, _) = space1(input)?;
    let (input, value) = take_while1(|c: char| !c.is_whitespace())(input)?;
    Ok((input, value.to_string()))
}

// Parses values like " 200" or " "<h1>Example</h1>" 200" or " "<h1>Example</h1>""
fn parse_respond_handler_args(input: &str) -> IResult<&str, (Option<u16>, Option<String>)> {
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

fn parse_redirect_handler_args(input: &str) -> IResult<&str, (Option<u16>, Option<String>)> {
    let (input, _) = space1(input)?;

    let (input, result) = alt((
        map(parse_string_u16, |(path, status_code)| {
            (Some(path.to_string()), Some(status_code))
        }),
        map(take_while1(|c: char| !c.is_whitespace()), |path: &str| {
            (Some(path.to_string()), None)
        }),
    ))(input)?;

    Ok((input, (result.1, result.0)))
}

// Parses the entire configuration, allowing comments and empty lines
/// Convert nom parsing errors into user-friendly error messages
fn format_parse_error(input: &str, error: nom::Err<Error<&str>>) -> String {
    match error {
        nom::Err::Error(e) | nom::Err::Failure(e) => {
            let error_location = find_error_location(input, e.input);
            let context = get_error_context(e.input);

            // Determine parsing context for more specific error messages
            let parsing_context = determine_parsing_context(input, e.input);

            match e.code {
                ErrorKind::Tag => {
                    if e.input.is_empty() {
                        "Unexpected end of file. The configuration appears to be incomplete."
                            .to_string()
                    } else {
                        let suggestion = get_context_specific_suggestion(&parsing_context, e.input);
                        format!(
                            "Syntax error near{}: '{}'. {}",
                            error_location, context, suggestion
                        )
                    }
                }
                ErrorKind::Char => {
                    let suggestion = get_context_specific_suggestion(&parsing_context, e.input);
                    format!(
                        "Expected a specific character near{}: '{}'. {}",
                        error_location, context, suggestion
                    )
                }
                ErrorKind::Alt => {
                    let suggestion = get_context_specific_suggestion(&parsing_context, e.input);
                    format!(
                        "Invalid syntax near{}: '{}'. {}",
                        error_location, context, suggestion
                    )
                }
                ErrorKind::Many1 => {
                    "Expected at least one virtual host definition in the configuration file."
                        .to_string()
                }
                _ => {
                    let suggestion = get_context_specific_suggestion(&parsing_context, e.input);
                    format!(
                        "Parse error near{}: '{}'. {}",
                        error_location, context, suggestion
                    )
                }
            }
        }
        nom::Err::Incomplete(_) => {
            "Configuration file appears to be incomplete or truncated.".to_string()
        }
    }
}

/// Determine what we were parsing when the error occurred
fn determine_parsing_context(full_input: &str, error_input: &str) -> String {
    let error_pos = full_input.len() - error_input.len();
    let before_error = &full_input[..error_pos];

    // Count braces to understand structure depth
    let open_braces = before_error.chars().filter(|&c| c == '{').count();
    let close_braces = before_error.chars().filter(|&c| c == '}').count();
    let brace_depth = open_braces.saturating_sub(close_braces);

    // Look for keywords in reverse order (most recent first)
    let mut found_route = false;
    let mut found_proxy = false;
    let mut found_handler = false;
    let mut found_middleware = false;

    // Check the last few "words" in the input before the error
    let words: Vec<&str> = before_error.split_whitespace().collect();
    let last_words: Vec<&str> = words.iter().rev().take(10).copied().collect();

    for &word in &last_words {
        if word == "route" {
            found_route = true;
            break;
        } else if word == "proxy"
            || word == "file"
            || word == "respond"
            || word == "redirect"
            || word == "dir"
            || word == "browse"
        {
            found_handler = true;
            break;
        } else if word == "upstreams" {
            found_proxy = true;
            break;
        } else if word == "gzip"
            || word == "cors"
            || word == "rate_limit"
            || word == "auth"
            || word == "cache"
            || word == "header"
        {
            found_middleware = true;
            break;
        }
    }

    // Determine context based on depth and keywords
    if brace_depth >= 2 && found_proxy {
        "proxy_upstreams".to_string()
    } else if brace_depth >= 2 && found_handler {
        "handler_definition".to_string()
    } else if brace_depth >= 2 && found_middleware {
        "middleware_definition".to_string()
    } else if brace_depth >= 2 && found_route {
        "route_contents".to_string()
    } else if brace_depth == 1 && found_route {
        "route_definition".to_string()
    } else if brace_depth == 1 {
        "virtual_host_contents".to_string()
    } else if found_route && brace_depth == 0 {
        "route_definition".to_string()
    } else {
        "virtual_host_definition".to_string()
    }
}

/// Get specific suggestions based on parsing context
fn get_context_specific_suggestion(context: &str, error_input: &str) -> String {
    let trimmed = error_input.trim();

    match context {
        "virtual_host_definition" => {
            if trimmed.chars().any(|c| c.is_alphabetic()) && !trimmed.contains('{') {
                "Domain definitions should be followed by a block enclosed in braces { }.".to_string()
            } else {
                "Expected domain name followed by configuration block. Example: 'example.com { ... }'.".to_string()
            }
        }
        "virtual_host_contents" => {
            "Virtual host block should contain route definitions. Example: 'route / { file index.html }'.".to_string()
        }
        "route_definition" => {
            if trimmed.starts_with("route") && !trimmed.contains('{') {
                "Route definitions should be followed by a block enclosed in braces { }.".to_string()
            } else {
                "Expected route path after 'route' keyword. Example: 'route /api { ... }'.".to_string()
            }
        }
        "route_contents" => {
            "Route block must contain at least one handler (file, proxy, respond, redirect, dir, browse).".to_string()
        }
        "handler_definition" => {
            if trimmed.starts_with("file") {
                "File handler requires a file path. Example: 'file index.html'.".to_string()
            } else if trimmed.starts_with("proxy") {
                "Proxy handler requires an upstream URL or block configuration. Example: 'proxy http://localhost:3000'.".to_string()
            } else if trimmed.starts_with("respond") {
                "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'.".to_string()
            } else if trimmed.starts_with("redirect") {
                "Redirect handler requires target path. Example: 'redirect /new-path'.".to_string()
            } else if trimmed.starts_with("dir") {
                "Directory handler requires a directory path. Example: 'dir /path/to/static'.".to_string()
            } else if trimmed.starts_with("browse") {
                "Browse handler requires a directory path. Example: 'browse /path/to/browse'.".to_string()
            } else {
                "Unknown handler type. Valid handlers: file, proxy, dir, browse, respond, redirect.".to_string()
            }
        }
        "proxy_handler" => {
            if trimmed.contains("upstreams") {
                "Proxy upstreams require valid URLs with protocol. Example: 'upstreams http://localhost:3000'.".to_string()
            } else {
                "Proxy configuration error. Use either 'proxy http://url' or 'proxy { upstreams ... }'.".to_string()
            }
        }
        "proxy_upstreams" => {
            "Invalid upstream URL. URLs must include protocol (http:// or https://).".to_string()
        }
        "middleware_definition" => {
            if trimmed.starts_with("rate_limit") {
                "Rate limit middleware requires a number. Example: 'rate_limit 10'.".to_string()
            } else if trimmed.starts_with("auth") {
                "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string()
            } else if trimmed.starts_with("cache") {
                "Cache middleware requires duration. Example: 'cache 5m'.".to_string()
            } else if trimmed.starts_with("header") {
                "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string()
            } else {
                "Unknown middleware type. Valid middleware: gzip, cors, log, rate_limit, auth, cache, header.".to_string()
            }
        }
        _ => suggest_fix_for_content(error_input),
    }
}

/// Find the approximate line and column number where the error occurred
fn find_error_location(full_input: &str, error_input: &str) -> String {
    // Calculate position where error occurred
    let error_pos = full_input.len() - error_input.len();

    // Count lines and find column
    let mut line = 1;
    let mut col = 1;

    for (i, ch) in full_input.chars().enumerate() {
        if i >= error_pos {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    format!(" line {}, column {}", line, col)
}

/// Get a snippet of context around the error location for display
fn get_error_context(error_input: &str) -> String {
    // Take the first 30 characters or until newline, whichever is shorter
    let context: String = error_input
        .chars()
        .take(30)
        .take_while(|&c| c != '\n')
        .collect();

    if context.len() < error_input.len() {
        format!("{}...", context)
    } else {
        context
    }
}

/// Provide suggestions for common configuration errors
fn suggest_fix_for_content(error_input: &str) -> String {
    let trimmed = error_input.trim();

    if trimmed.starts_with('{') && !trimmed.contains('}') {
        "Check for missing closing brace '}'.".to_string()
    } else if trimmed.contains("route") && !trimmed.contains('{') {
        "Route definitions should be followed by a block enclosed in braces { }.".to_string()
    } else if trimmed.chars().any(|c| c.is_alphabetic()) && !trimmed.contains('{') {
        "Domain definitions should be followed by a block enclosed in braces { }.".to_string()
    } else if trimmed.starts_with("proxy")
        || trimmed.starts_with("file")
        || trimmed.starts_with("respond")
    {
        "Handler definitions should be inside a route block.".to_string()
    } else if trimmed.is_empty() {
        "Configuration file appears to be empty or contains only whitespace.".to_string()
    } else {
        "Check the configuration syntax - ensure domains, routes, and handlers are properly defined.".to_string()
    }
}

pub fn parse_config(input: &str) -> Result<(&str, Config), String> {
    let result: Result<(&str, Vec<VirtualHost>), Err<Error<&str>>> = many1(alt((
        map(parse_virtual_host, Some),
        map(parse_comment, |_| None), // Skip comments
    )))(input)
    .map(|(i, hosts)| (i, hosts.into_iter().flatten().collect()));

    match result {
        Ok(r) => Ok((r.0, Config { virtual_hosts: r.1 })),
        Err(e) => Err(format_parse_error(input, e)),
    }
}

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
    let (input, digits) = take_while1(|c: char| !c.is_whitespace())(input)?;
    let (remaining, digits) = digit1(digits)?;

    // Ensure there are no additional characters after the digits
    if !remaining.is_empty() {
        return Err(Err::Error(Error::new(input, ErrorKind::Digit)));
    }

    // Convert the digits string to a u16
    // This will return an error if the value is too large for u16
    let value = digits
        .parse::<u16>()
        .map_err(|_| nom::Err::Error((input, nom::error::ErrorKind::Digit)));

    match value {
        Ok(v) => Ok((input, v)),
        Err(_e) => Err(Err::Error(Error::new(input, ErrorKind::Digit))),
    }
}

/// Parses a string literal and an unsigned 16-bit integer (u16) example: "Some String" 123
fn parse_literal_u16(input: &str) -> IResult<&str, (String, u16)> {
    tuple((string_literal, preceded(space1, parse_u16)))(input)
}

/// parse string and unsigned 16-bit integer (u16) example: sometext 123
fn parse_string_u16(input: &str) -> IResult<&str, (&str, u16)> {
    tuple((
        take_while1(|c: char| !c.is_whitespace()),
        preceded(space1, parse_u16),
    ))(input)
}

#[cfg(test)]
mod tests {

    mod comments {
        use crate::parse_comment;

        #[test]
        fn test_parse_comment_success() {
            assert_eq!(parse_comment("# this is a comment"), Ok(("", ())));
            assert_eq!(parse_comment("# this is a comment\n"), Ok(("\n", ())));
            assert_eq!(parse_comment("# this is a comment\n\n"), Ok(("\n\n", ())));
            assert_eq!(
                parse_comment("# this is a comment\n\n\n"),
                Ok(("\n\n\n", ()))
            );
            // 1 space before comment
            assert_eq!(parse_comment(" # this is a comment"), Ok(("", ())));
            // 2 spaces before comment
            assert_eq!(parse_comment("  # this is a comment"), Ok(("", ())));
            // 3 spaces before comment
            assert_eq!(parse_comment("   # this is a comment"), Ok(("", ())));
            // 4 spaces before comment
            assert_eq!(parse_comment("    # this is a comment"), Ok(("", ())));
            assert_eq!(parse_comment("\t# this is a comment"), Ok(("", ())));
            assert_eq!(parse_comment("\t # this is a comment"), Ok(("", ())));
            assert_eq!(parse_comment("\t\t # this is a comment"), Ok(("", ())));
            assert_eq!(parse_comment("\t\t  # this is a comment"), Ok(("", ())));
        }

        #[test]
        fn test_parse_comment_fail() {
            assert!(parse_comment("this is not a comment").is_err());
            assert!(parse_comment("this is not a comment\n").is_err());
            assert!(parse_comment("this is not a comment\n\n").is_err());
            assert!(parse_comment("this is not a comment\n\n\n").is_err());
            assert!(parse_comment("this is not a comment\n\n\n\n").is_err());
            assert!(parse_comment("this is not a comment\n\n\n\n\n").is_err());
            assert!(parse_comment("this is not a comment\n\n\n\n\n\n").is_err());
            assert!(parse_comment("this is not a comment\n\n\n\n\n\n\n").is_err());
        }
    }

    mod routes {
        use crate::{parse_route, parse_route_contents, types};

        #[test]
        fn test_parse_route_respond_handler_with_no_middleware_inline() {
            assert_eq!(
                parse_route("route /example { respond \"<h1>Example</h1>\" 200 }"),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: Some(200),
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        middlewares: vec![]
                    }),
                ))
            );

            assert_eq!(
                parse_route("route /example { respond 200 }"),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: Some(200),
                            body: None,
                        },
                        middlewares: vec![]
                    }),
                ))
            );

            assert_eq!(
                parse_route("route /example { respond \"<h1>Example</h1>\" }"),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: None,
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        middlewares: vec![]
                    }),
                ))
            );
        }

        #[test]
        fn test_parse_route_respond_handler_with_no_middleware_expanded() {
            let route = r#"
            route /example {
                respond "<h1>Example</h1>" 200
            }
            "#;

            assert_eq!(
                parse_route(route),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: Some(200),
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        middlewares: vec![]
                    }),
                ))
            );

            let route = r#"
            route /example {
                respond 200
            }
            "#;

            assert_eq!(
                parse_route(route),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: Some(200),
                            body: None,
                        },
                        middlewares: vec![]
                    }),
                ))
            );

            let route = r#"
            route /example {
                respond "<h1>Example</h1>"
            }
            "#;

            assert_eq!(
                parse_route(route),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: None,
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        middlewares: vec![]
                    }),
                ))
            );
        }

        #[test]
        fn test_parse_route_file_handler_with_no_middleware_inline() {
            assert_eq!(
                parse_route("route / { file index.html }"),
                Ok((
                    "",
                    Some(types::Route {
                        handler: types::Handler::File("index.html".to_string()),
                        middlewares: vec![],
                        path: "/".to_string(),
                    }),
                ))
            )
        }

        #[test]
        fn test_parse_route_file_handler_with_no_middleware_expanded() {
            let route = r#"
            route / {
                file index.html
            }
            "#;
            assert_eq!(
                parse_route(route),
                Ok((
                    "",
                    Some(types::Route {
                        handler: types::Handler::File("index.html".to_string()),
                        middlewares: vec![],
                        path: "/".to_string(),
                    }),
                ))
            )
        }

        #[test]
        fn test_parse_route_with_middleware() {
            let route = r#"
            route /example {
            respond "<h1>Example</h1>" 200
            gzip
            cors
            }
            "#;

            assert_eq!(
                parse_route(route),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: Some(200),
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        middlewares: vec![types::Middleware::Gzip, types::Middleware::Cors,]
                    }),
                ))
            );
        }

        #[test]
        fn test_parse_route_with_comments() {
            let route = r#"
            # This is a comment
            route /example {
            # Another comment
            respond "<h1>Example</h1>" 200
            # Middleware comment
            gzip
            }
            "#;

            assert_eq!(
                parse_route(route),
                Ok((
                    "",
                    Some(types::Route {
                        path: "/example".to_string(),
                        handler: types::Handler::Respond {
                            status: Some(200),
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        middlewares: vec![types::Middleware::Gzip,]
                    }),
                ))
            );
        }

        #[test]
        fn test_parse_route_contents_with_middleware() {
            let contents = r#"
            respond "<h1>Example</h1>" 200
            gzip
            cors
            "#;

            assert_eq!(
                parse_route_contents(contents),
                Ok((
                    "",
                    (
                        types::Handler::Respond {
                            status: Some(200),
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        vec![types::Middleware::Gzip, types::Middleware::Cors,]
                    )
                ))
            );
        }

        #[test]
        fn test_parse_route_contents_with_comments() {
            let contents = r#"
            # This is a comment
            respond "<h1>Example</h1>" 200
            # Middleware comment
            gzip
            "#;

            assert_eq!(
                parse_route_contents(contents),
                Ok((
                    "",
                    (
                        types::Handler::Respond {
                            status: Some(200),
                            body: Some("<h1>Example</h1>".to_string()),
                        },
                        vec![types::Middleware::Gzip,]
                    )
                ))
            );
        }
    }

    mod handlers {
        use crate::{
            parse_handler, parse_redirect_handler_args, parse_respond_handler_args,
            types::{self, Upstream},
        };

        #[test]
        fn test_parse_handler_file() {
            assert_eq!(
                parse_handler("file index.html"),
                Ok(("", types::Handler::File("index.html".to_string())))
            );
        }

        #[test]
        fn test_parse_handler_proxy() {
            assert_eq!(
                parse_handler("proxy http://localhost:3000"),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::NoBalancer(
                        Upstream::new("http://localhost:3000".to_string()).unwrap()
                    ))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_single_upstream() {
            let input = "proxy { upstreams http://localhost:3000 }";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::NoBalancer(
                        Upstream::new("http://localhost:3000".to_string()).unwrap()
                    ))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_multiple_upstreams_no_policy() {
            let input = "proxy { upstreams http://host1:8080 http://host2:8080 }";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::RoundRobin(vec![
                        Upstream::new("http://host1:8080".to_string()).unwrap(),
                        Upstream::new("http://host2:8080".to_string()).unwrap(),
                    ]))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_multiple_upstreams_round_robin() {
            let input = "proxy { upstreams http://host1:8080 http://host2:8080 http://host3:8080\n lb_policy round_robin }";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::RoundRobin(vec![
                        Upstream::new("http://host1:8080".to_string()).unwrap(),
                        Upstream::new("http://host2:8080".to_string()).unwrap(),
                        Upstream::new("http://host3:8080".to_string()).unwrap(),
                    ]))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_single_upstream_round_robin() {
            let input = "proxy { upstreams http://localhost:3000\n lb_policy round_robin }";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::NoBalancer(
                        Upstream::new("http://localhost:3000".to_string()).unwrap()
                    ))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_empty_lb_policy() {
            let input = "proxy { upstreams http://host1:8080 http://host2:8080\n lb_policy }";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::RoundRobin(vec![
                        Upstream::new("http://host1:8080".to_string()).unwrap(),
                        Upstream::new("http://host2:8080".to_string()).unwrap(),
                    ]))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_whitespace_handling() {
            let input = "proxy {\n  upstreams  http://host1:8080   http://host2:8080  \n  lb_policy   round_robin  \n}";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::RoundRobin(vec![
                        Upstream::new("http://host1:8080".to_string()).unwrap(),
                        Upstream::new("http://host2:8080".to_string()).unwrap(),
                    ]))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_with_comments() {
            let input = "proxy {\n  # Comment before upstreams\n  upstreams http://host1:8080 http://host2:8080\n  # Comment before lb_policy\n  lb_policy round_robin\n  # Comment after lb_policy\n}";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::RoundRobin(vec![
                        Upstream::new("http://host1:8080".to_string()).unwrap(),
                        Upstream::new("http://host2:8080".to_string()).unwrap(),
                    ]))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_single_upstream_with_comments() {
            let input = "proxy {\n  # This is a comment\n  upstreams http://localhost:3000\n  # Another comment\n}";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::LoadBalancer::NoBalancer(
                        Upstream::new("http://localhost:3000".to_string()).unwrap()
                    ))
                ))
            );
        }

        #[test]
        fn test_parse_handler_browse() {
            assert_eq!(
                parse_handler("browse /path/to/dir"),
                Ok(("", types::Handler::Browse("/path/to/dir".to_string())))
            );
        }

        #[test]
        fn test_parse_handler_dir() {
            assert_eq!(
                parse_handler("dir /path/to/dir"),
                Ok(("", types::Handler::Dir("/path/to/dir".to_string())))
            );
        }

        #[test]
        fn test_parse_handler_respond() {
            assert_eq!(
                parse_handler("respond \"<h1>Example</h1>\" 200"),
                Ok((
                    "",
                    types::Handler::Respond {
                        status: Some(200),
                        body: Some("<h1>Example</h1>".to_string()),
                    }
                ))
            );

            assert_eq!(
                parse_handler("respond \"<h1>Example</h1>\""),
                Ok((
                    "",
                    types::Handler::Respond {
                        status: None,
                        body: Some("<h1>Example</h1>".to_string()),
                    }
                ))
            );

            assert_eq!(
                parse_handler("respond 200"),
                Ok((
                    "",
                    types::Handler::Respond {
                        status: Some(200),
                        body: None,
                    }
                ))
            );
        }

        #[test]
        fn test_parse_handler_redirect() {
            assert_eq!(
                parse_handler("redirect /new-path 301"),
                Ok((
                    "",
                    types::Handler::Redirect {
                        status_code: Some(301),
                        path: Some("/new-path".to_string())
                    }
                ))
            );

            assert_eq!(
                parse_handler("redirect /new-path"),
                Ok((
                    "",
                    types::Handler::Redirect {
                        status_code: None,
                        path: Some("/new-path".to_string())
                    }
                ))
            );
        }

        #[test]
        fn test_parse_respond_handler_args() {
            // test with body
            assert_eq!(
                parse_respond_handler_args(" \"<h1>Example</h1>\""),
                Ok(("", (None, Some("<h1>Example</h1>".to_string()))))
            );
            // test with body and status code
            assert_eq!(
                parse_respond_handler_args(" \"<h1>Example</h1>\" 200"),
                Ok(("", (Some(200), Some("<h1>Example</h1>".to_string()))))
            );

            // test with status code
            assert_eq!(
                parse_respond_handler_args(" 200"),
                Ok(("", (Some(200), None)))
            );
        }

        #[test]
        fn test_parse_redirect_handler_args() {
            // test with path
            assert_eq!(
                parse_redirect_handler_args(" /path/to/redirect"),
                Ok(("", (None, Some("/path/to/redirect".to_string()))))
            );

            // test with path and status code
            assert_eq!(
                parse_redirect_handler_args(" /path/to/redirect 301"),
                Ok(("", (Some(301), Some("/path/to/redirect".to_string()))))
            );
        }
    }

    mod middlewares {
        use crate::{parse_auth, parse_cache, parse_header, parse_middleware, types};
        use rstest::rstest;
        #[test]
        fn test_parse_middleware_gzip() {
            assert_eq!(parse_middleware("gzip"), Ok(("", types::Middleware::Gzip)));
        }

        #[test]
        fn test_parse_middleware_cors() {
            assert_eq!(parse_middleware("cors"), Ok(("", types::Middleware::Cors)));
        }

        #[test]
        fn test_parse_middleware_log() {
            assert_eq!(parse_middleware("log"), Ok(("", types::Middleware::Log)));
        }

        #[test]
        fn test_parse_middleware_rate_limit() {
            assert_eq!(
                parse_middleware("rate_limit 10"),
                Ok(("", types::Middleware::RateLimit(10)))
            );
        }

        #[test]
        fn test_parse_middleware_auth() {
            assert_eq!(
                parse_middleware("auth admin pass"),
                Ok((
                    "",
                    types::Middleware::Auth {
                        username: "admin".to_string(),
                        password: "pass".to_string()
                    }
                ))
            );
        }

        #[test]
        fn test_parse_middleware_cache() {
            assert_eq!(
                parse_middleware("cache 5m"),
                Ok(("", types::Middleware::Cache("5m".to_string())))
            );
        }

        #[rstest]
        #[case(
            "header +X-Cache HIT",
            types::HeaderOperator::Add,
            "X-Cache",
            Some("HIT"),
            None
        )]
        #[case("header -Server", types::HeaderOperator::Delete, "Server", None, None)]
        #[case(
            "header =Content-Type text/html",
            types::HeaderOperator::Set,
            "Content-Type",
            Some("text/html"),
            None
        )]
        #[case(
            "header >Content-Type text/html",
            types::HeaderOperator::DeferSet,
            "Content-Type",
            Some("text/html"),
            None
        )]
        #[case(
            "header ~Location http:// https://",
            types::HeaderOperator::Replace,
            "Location",
            Some("http://"),
            Some("https://")
        )]
        #[case(
            "header ~>Location http:// https://",
            types::HeaderOperator::DeferReplace,
            "Location",
            Some("http://"),
            Some("https://")
        )]
        #[case(
            "header ?Cache-Control max-age=3600",
            types::HeaderOperator::Default,
            "Cache-Control",
            Some("max-age=3600"),
            None
        )]
        fn test_parse_middleware_header(
            #[case] input: &str,
            #[case] operator: types::HeaderOperator,
            #[case] name: &str,
            #[case] value: Option<&str>,
            #[case] replace_with: Option<&str>,
        ) {
            assert_eq!(
                parse_middleware(input),
                Ok((
                    "",
                    types::Middleware::Header {
                        operator: operator.clone(),
                        name: name.to_string(),
                        value: value.map(|s| s.to_string()),
                        replace_with: replace_with.map(|s| s.to_string()),
                    }
                ))
            );

            assert_eq!(
                parse_header(input),
                Ok((
                    "",
                    types::Middleware::Header {
                        operator,
                        name: name.to_string(),
                        value: value.map(|s| s.to_string()),
                        replace_with: replace_with.map(|s| s.to_string()),
                    }
                ))
            );
        }

        #[test]
        fn test_parse_cache() {
            assert_eq!(
                parse_cache("cache 5m"),
                Ok(("", types::Middleware::Cache("5m".to_string())))
            );
        }

        #[test]
        fn test_parse_auth() {
            assert_eq!(
                parse_auth("auth admin pass"),
                Ok((
                    "",
                    types::Middleware::Auth {
                        username: "admin".to_string(),
                        password: "pass".to_string()
                    }
                ))
            );
        }

        #[test]
        fn test_parse_rate_limit() {
            assert_eq!(
                crate::parse_rate_limit("rate_limit 10"),
                Ok(("", types::Middleware::RateLimit(10)))
            );
        }
    }

    mod utils {
        use crate::{parse_literal_u16, parse_string_u16, parse_u16, string_literal};

        #[test]
        fn test_parse_string_u16_success() {
            assert_eq!(
                parse_string_u16("http://localhost:3000 200"),
                Ok(("", ("http://localhost:3000", 200)))
            );
            assert_eq!(parse_string_u16("/blog 403"), Ok(("", ("/blog", 403))));
            assert_eq!(parse_string_u16("** 101"), Ok(("", ("**", 101))));
            assert_eq!(parse_string_u16("{value} 404"), Ok(("", ("{value}", 404))));
            assert_eq!(
                parse_string_u16("about-us 301"),
                Ok(("", ("about-us", 301)))
            );
        }

        #[test]
        fn test_parse_string_u16_failure() {
            assert!(parse_string_u16("").is_err());
            assert!(parse_string_u16(" ").is_err());
            assert!(parse_string_u16("http://localhost:3000").is_err());
            assert!(parse_string_u16("3000").is_err());
            assert!(parse_string_u16("http://localhost:3000 abc").is_err());
            assert!(parse_string_u16("http://localhost:3000 -200").is_err());
        }

        #[test]
        fn test_string_literal_success() {
            assert_eq!(string_literal("\"hello\""), Ok(("", "hello".to_string())));
            assert_eq!(string_literal("\"world\""), Ok(("", "world".to_string())));
            assert_eq!(string_literal("\"12345\""), Ok(("", "12345".to_string())));
            assert_eq!(string_literal("\"!@#$%\""), Ok(("", "!@#$%".to_string())));
            assert_eq!(
                string_literal("\"with spaces\""),
                Ok(("", "with spaces".to_string()))
            );
        }

        #[test]
        fn test_string_literal_failure() {
            assert!(string_literal("hello").is_err());
            assert!(string_literal("\"unclosed").is_err());
            assert!(string_literal("unopened\"").is_err());
            assert!(string_literal("\"mismatched'").is_err());
            assert!(string_literal("").is_err());
        }

        #[test]
        fn test_parse_literal_u16_success() {
            assert_eq!(
                parse_literal_u16("\"<h1>Example</h1>\" 200"),
                Ok(("", ("<h1>Example</h1>".to_string(), 200)))
            );
            assert_eq!(
                parse_literal_u16("\"Hello, World!\" 404"),
                Ok(("", ("Hello, World!".to_string(), 404)))
            );
            assert_eq!(
                parse_literal_u16("\"Test String\" 500"),
                Ok(("", ("Test String".to_string(), 500)))
            );
        }

        #[test]
        fn test_parse_literal_u16_failure() {
            assert!(parse_literal_u16("").is_err());
            assert!(parse_literal_u16(" ").is_err());
            assert!(parse_literal_u16("\"Unclosed").is_err());
            assert!(parse_literal_u16("Unopened\"").is_err());
            assert!(parse_literal_u16("\"Mismatched' 200").is_err());
            assert!(parse_literal_u16("\"Valid String\" -200").is_err());
            assert!(parse_literal_u16("\"Valid String\" abc").is_err());
        }

        #[test]
        fn test_parse_u16_success() {
            assert_eq!(parse_u16("123"), Ok(("", 123)));
            assert_eq!(parse_u16("0"), Ok(("", 0)));
            assert_eq!(parse_u16("65535"), Ok(("", 65535)));
            assert_eq!(parse_u16("  42"), Ok(("", 42)));
            assert_eq!(parse_u16("\n99"), Ok(("", 99)));
        }

        #[test]
        fn test_parse_u16_failure() {
            assert!(parse_u16("").is_err());
            assert!(parse_u16(" ").is_err());
            assert!(parse_u16("abc").is_err());
            assert!(parse_u16("-123").is_err());
            assert!(parse_u16("123456").is_err()); // Out of range for u16
            assert!(parse_u16("12.34").is_err());
        }
    }

    mod values {
        use crate::parse_value;
        #[test]
        fn test_parse_value_success() {
            assert_eq!(
                parse_value(" index.html"),
                Ok(("", "index.html".to_string()))
            );
            assert_eq!(
                parse_value(" http://localhost:3000"),
                Ok(("", "http://localhost:3000".to_string()))
            );
            assert_eq!(
                parse_value(" /path/to/file"),
                Ok(("", "/path/to/file".to_string()))
            );
            assert_eq!(
                parse_value(" some_value"),
                Ok(("", "some_value".to_string()))
            );
        }

        #[test]
        fn test_parse_value_failure() {
            assert!(parse_value("").is_err());
            assert!(parse_value(" ").is_err());
            assert!(parse_value("\t").is_err());
            assert!(parse_value("\n").is_err());
        }
    }

    mod virtual_host {
        use crate::parse_virtual_host;
        use crate::types;

        #[test]
        fn test_parse_virtual_host_success() {
            let input = r#"
                example.com {
                    route / {
                        file index.html
                    }
                }
                "#;

            assert_eq!(
                parse_virtual_host(input),
                Ok((
                    "\n                ",
                    types::VirtualHost {
                        domain: "example.com".to_string(),
                        routes: vec![types::Route {
                            path: "/".to_string(),
                            handler: types::Handler::File("index.html".to_string()),
                            middlewares: vec![],
                        }],
                    }
                ))
            );
        }

        #[test]
        fn test_parse_virtual_host_with_multiple_routes() {
            let input = r#"
                example.com {
                    route / {
                        file index.html
                    }
                    route /about {
                        file about.html
                    }
                }
                "#;

            assert_eq!(
                parse_virtual_host(input),
                Ok((
                    "\n                ",
                    types::VirtualHost {
                        domain: "example.com".to_string(),
                        routes: vec![
                            types::Route {
                                path: "/".to_string(),
                                handler: types::Handler::File("index.html".to_string()),
                                middlewares: vec![],
                            },
                            types::Route {
                                path: "/about".to_string(),
                                handler: types::Handler::File("about.html".to_string()),
                                middlewares: vec![],
                            },
                        ],
                    }
                ))
            );
        }

        #[test]
        fn test_parse_virtual_host_with_comments() {
            let input = r#"
                example.com {
                    # Another comment
                    route / {
                        file index.html
                    }
                    # Comment between routes
                    route /about {
                        file about.html
                    }
                }
                "#;

            assert_eq!(
                parse_virtual_host(input),
                Ok((
                    "\n                ",
                    types::VirtualHost {
                        domain: "example.com".to_string(),
                        routes: vec![
                            types::Route {
                                path: "/".to_string(),
                                handler: types::Handler::File("index.html".to_string()),
                                middlewares: vec![],
                            },
                            types::Route {
                                path: "/about".to_string(),
                                handler: types::Handler::File("about.html".to_string()),
                                middlewares: vec![],
                            },
                        ],
                    }
                ))
            );
        }

        #[test]
        fn test_parse_virtual_host_with_middleware() {
            let input = r#"
                example.com {
                    route / {
                        file index.html
                        gzip
                        cors
                    }
                }
                "#;

            assert_eq!(
                parse_virtual_host(input),
                Ok((
                    "\n                ",
                    types::VirtualHost {
                        domain: "example.com".to_string(),
                        routes: vec![types::Route {
                            path: "/".to_string(),
                            handler: types::Handler::File("index.html".to_string()),
                            middlewares: vec![types::Middleware::Gzip, types::Middleware::Cors],
                        }],
                    }
                ))
            );
        }

        #[test]
        fn test_parse_virtual_host_failure() {
            let input = r#"
                example.com {
                    route / {
                        file index.html
                    }
                "#; // Missing closing brace

            assert!(parse_virtual_host(input).is_err());
        }
    }

    mod config {
        use crate::{
            parse_config,
            types::{self, Config, Upstream},
        };

        #[test]
        fn test_parse_config_single_virtual_host() {
            let input = r#"
            example.com {
                route / {
                    file index.html
                }
            }
            "#;

            assert_eq!(
                parse_config(input),
                Ok((
                    "\n            ",
                    Config {
                        virtual_hosts: vec![types::VirtualHost {
                            domain: "example.com".to_string(),
                            routes: vec![types::Route {
                                path: "/".to_string(),
                                handler: types::Handler::File("index.html".to_string()),
                                middlewares: vec![],
                            }],
                        }]
                    }
                ))
            );
        }

        #[test]
        fn test_parse_config_multiple_virtual_hosts() {
            let input = r#"
            example.com {
                route / {
                    file index.html
                }
            }
            another.com {
                route /about {
                    file about.html
                }
            }
            "#;

            assert_eq!(
                parse_config(input),
                Ok((
                    "\n            ",
                    Config {
                        virtual_hosts: vec![
                            types::VirtualHost {
                                domain: "example.com".to_string(),
                                routes: vec![types::Route {
                                    path: "/".to_string(),
                                    handler: types::Handler::File("index.html".to_string()),
                                    middlewares: vec![],
                                }],
                            },
                            types::VirtualHost {
                                domain: "another.com".to_string(),
                                routes: vec![types::Route {
                                    path: "/about".to_string(),
                                    handler: types::Handler::File("about.html".to_string()),
                                    middlewares: vec![],
                                }],
                            }
                        ]
                    }
                ))
            );
        }

        #[test]
        fn test_parse_config_with_comments() {
            let input = r#"
            # This is a comment
            example.com {
                # Another comment
                route / {
                    file index.html
                }
            }
            another.com {
                route /about {
                    file about.html
                }
            }
            "#;

            assert_eq!(
                parse_config(input),
                Ok((
                    "\n            ",
                    Config {
                        virtual_hosts: vec![
                            types::VirtualHost {
                                domain: "example.com".to_string(),
                                routes: vec![types::Route {
                                    path: "/".to_string(),
                                    handler: types::Handler::File("index.html".to_string()),
                                    middlewares: vec![],
                                }],
                            },
                            types::VirtualHost {
                                domain: "another.com".to_string(),
                                routes: vec![types::Route {
                                    path: "/about".to_string(),
                                    handler: types::Handler::File("about.html".to_string()),
                                    middlewares: vec![],
                                }],
                            }
                        ]
                    }
                ))
            );
        }

        #[test]
        fn test_parse_config_with_new_proxy_syntax_and_comments() {
            let config_str = r#"
            # Server with new proxy syntax and comments
            localhost {
                # Old syntax (backward compatibility)  
                route /old-proxy {
                    # Inline comment
                    proxy http://old-upstream:3000 # This is a comment
                }
                
                # New syntax - single upstream with comments
                route /single-proxy {
                    proxy {
                        # Comment before upstreams
                        upstreams http://new-upstream:4000
                        # Comment after single upstream
                    }
                }
                
                # New syntax - multiple upstreams with comments  
                route /multi-proxy {
                    proxy {
                        # This proxy has multiple upstreams
                        upstreams http://backend1:5000 http://backend2:5000 http://backend3:5000  
                        # Load balancing policy
                        lb_policy round_robin
                        # End of proxy config
                    }
                }
                
                # New syntax - multiple upstreams with comments on separate lines
                route /multi-proxy-2 {
                    proxy {
                        # Multiple upstreams with inline comments  
                        upstreams http://backend4:6000 # first server
                                 http://backend5:6000 # second server
                        # Auto round robin since multiple upstreams
                    }
                }
            }
            "#;

            let result = parse_config(config_str);
            assert!(result.is_ok());

            let (_, config) = result.unwrap();
            assert_eq!(config.virtual_hosts.len(), 1);

            let vh = &config.virtual_hosts[0];
            assert_eq!(vh.domain, "localhost");
            assert_eq!(vh.routes.len(), 4);

            // Check old syntax route
            let old_route = &vh.routes[0];
            assert_eq!(old_route.path, "/old-proxy");
            assert!(matches!(
                old_route.handler,
                types::Handler::Proxy(types::LoadBalancer::NoBalancer(_))
            ));

            // Check single upstream with comments route
            let single_route = &vh.routes[1];
            assert_eq!(single_route.path, "/single-proxy");
            assert!(matches!(
                single_route.handler,
                types::Handler::Proxy(types::LoadBalancer::NoBalancer(_))
            ));

            // Check multi upstream with explicit round_robin
            let multi_route = &vh.routes[2];
            assert_eq!(multi_route.path, "/multi-proxy");
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &multi_route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer");
            }

            // Check the second multi upstream route
            let multi_route_2 = &vh.routes[3];
            assert_eq!(multi_route_2.path, "/multi-proxy-2");
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &multi_route_2.handler
            {
                assert_eq!(upstreams.len(), 2);
            } else {
                panic!("Expected RoundRobin load balancer for multiple upstreams");
            }
        }

        #[test]
        fn test_parse_config_with_middleware() {
            let input = r#"
            example.com {
                route / {
                    file index.html
                    gzip
                    cors
                }
            }
            "#;

            assert_eq!(
                parse_config(input),
                Ok((
                    "\n            ",
                    Config {
                        virtual_hosts: vec![types::VirtualHost {
                            domain: "example.com".to_string(),
                            routes: vec![types::Route {
                                path: "/".to_string(),
                                handler: types::Handler::File("index.html".to_string()),
                                middlewares: vec![types::Middleware::Gzip, types::Middleware::Cors],
                            }],
                        }]
                    }
                ))
            );
        }

        #[test]
        fn test_parse_config_failure() {
            let input = r#"
            example.com {
                route / {
                    file index.html
                }
            "#; // Missing closing brace

            assert!(parse_config(input).is_err());
        }

        #[test]
        fn test_multiline_upstream_basic() {
            let input = r#"
        localhost {
            route /api/* {
                proxy {
                    upstreams http://backend1:8080
                             http://backend2:8080
                             http://backend3:8080
                    lb_policy round_robin
                }
            }
        }
        "#;
            let result = parse_config(input);
            assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

            let (_, config) = result.unwrap();
            assert_eq!(config.virtual_hosts.len(), 1);

            let route = &config.virtual_hosts[0].routes[0];
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer with 3 upstreams");
            }
        }

        #[test]
        fn test_multiline_upstream_with_comments() {
            let input = r#"
        localhost {
            route /api/* {
                proxy {
                    upstreams http://backend1:8080  # First backend
                             http://backend2:8080  # Second backend
                             http://backend3:8080  # Third backend
                    lb_policy round_robin
                }
            }
        }
        "#;
            let result = parse_config(input);
            assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

            let (_, config) = result.unwrap();
            let route = &config.virtual_hosts[0].routes[0];
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer with 3 upstreams");
            }
        }

        #[test]
        fn test_multiline_upstream_mixed_with_newlines() {
            let input = r#"
        localhost {
            route /api/* {
                proxy {
                    upstreams http://backend1:8080

                             http://backend2:8080
                             
                             http://backend3:8080
                             
                    lb_policy round_robin
                }
            }
        }
        "#;
            let result = parse_config(input);
            assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

            let (_, config) = result.unwrap();
            let route = &config.virtual_hosts[0].routes[0];
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer with 3 upstreams");
            }
        }

        #[test]
        fn test_multiline_upstream_different_indentation() {
            let input = r#"
        localhost {
            route /api/* {
                proxy {
                    upstreams http://backend1:8080
        http://backend2:8080
            http://backend3:8080
                    lb_policy round_robin
                }
            }
        }
        "#;
            let result = parse_config(input);
            assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

            let (_, config) = result.unwrap();
            let route = &config.virtual_hosts[0].routes[0];
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer with 3 upstreams");
            }
        }

        #[test]
        fn test_multiline_upstream_one_per_line_with_first_upstream() {
            let input = r#"
        localhost {
            route /api/* {
                proxy {
                    upstreams http://backend1:8080
                              http://backend2:8080
                              http://backend3:8080
                }
            }
        }
        "#;
            let result = parse_config(input);
            assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

            let (_, config) = result.unwrap();
            let route = &config.virtual_hosts[0].routes[0];
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer with 3 upstreams");
            }
        }

        #[test]
        fn test_multiline_upstream_tab_indentation() {
            let input = "
        localhost {
            route /api/* {
                proxy {
                    upstreams http://backend1:8080
\t\t\t\t              http://backend2:8080
\t\t\t\t              http://backend3:8080
                    lb_policy round_robin
                }
            }
        }
        ";
            let result = parse_config(input);
            assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

            let (_, config) = result.unwrap();
            let route = &config.virtual_hosts[0].routes[0];
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer with 3 upstreams");
            }
        }

        #[test]
        fn test_multiline_upstream_comments_between_lines() {
            let input = r#"
        localhost {
            route /api/* {
                proxy {
                    upstreams http://backend1:8080
                              # Comment between upstreams  
                              http://backend2:8080
                              # Another comment
                              http://backend3:8080
                    # Comment before policy
                    lb_policy round_robin
                }
            }
        }
        "#;
            let result = parse_config(input);
            assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

            let (_, config) = result.unwrap();
            let route = &config.virtual_hosts[0].routes[0];
            if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                &route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer with 3 upstreams");
            }
        }

        #[test]
        fn test_multiline_upstream_upstreams_on_separate_line() {
            let input = r#"
        localhost {
            route /api/* {
                proxy {
                    upstreams 
                        http://backend1:8080
                        http://backend2:8080
                        http://backend3:8080
                    lb_policy round_robin
                }
            }
        }
        "#;
            // Let's see what happens - maybe the parser supports this
            let result = parse_config(input);
            if result.is_ok() {
                let (_, config) = result.unwrap();
                let route = &config.virtual_hosts[0].routes[0];
                if let types::Handler::Proxy(types::LoadBalancer::RoundRobin(upstreams)) =
                    &route.handler
                {
                    assert_eq!(upstreams.len(), 3);
                    println!(
                        "Success: parsed {} upstreams when upstreams keyword is on separate line",
                        upstreams.len()
                    );
                } else {
                    panic!("Expected RoundRobin load balancer with upstreams");
                }
            } else {
                panic!("Parsing failed: {:?}", result.err());
            }
        }

        #[test]
        fn test_parse_config_full() {
            let input = r#"
    # This is comment
    # This is comment

    # This is comment
    localhost {
        # This is comment
        route / {
            # This is comment
            file index.html
            # This is comment
            gzip
            # This is comment
            log 
            auth admin password123
            cache 30s # This is comment
            # This is comment
        }
        # This is comment
        route /api/** {
            # This is comment
            proxy http://localhost:3000 # This is comment
            cors
            # This is comment
            rate_limit 10 
        }

        route /static-response {
            # This is comment
            respond "Hello, world!" # This is comment
        }

        # This is comment
        route /health {
            respond 200 # This is comment
        }

        # This is comment
        route /secret {
            respond "Access Denied" 403 # This is comment
        }

        # This is comment
        route /old-path {
            redirect /new-path
        }

        # This is comment
        route /old-path-with-status {
            redirect /new-path 301
        }

        route /example {
            respond "<h1>Example</h1>" 200
            
            #header +Content-Type text/html

            header =X-Set-Or-Overwrite-Example-Header value
            header >X-Set-With-defer value
            header -X-Delete-Example-Header
            header +X-Add-Example-Header value
            header ?X-Set-If-NotExist-Example-Header value 
            header ~X-Replace-Header-Value value replace_with_this
            header ~>X-Replace-Header-Value-With-Defer value replace_with_this

        }

        # This is comment
        # This is comment

    }
    # This is comment
    example.com {
        # This is comment

        route /blog/** {
        # This is comment

            proxy http://blog.example.com
            gzip
            cache 5m
        # This is comment

        }
        # This is comment
        
        route /admin {
        # This is comment

            proxy http://admin.example.com
        # This is comment

            auth superuser secret
        # This is comment

        }
        # This is comment

    }
"#;
            assert_eq!(
                parse_config(input),
                Ok((
                    "\n",
                    Config {
                        virtual_hosts: vec![
                            types::VirtualHost {
                                domain: "localhost".to_string(),
                                routes: vec![
                                    types::Route {
                                        path: "/".to_string(),
                                        handler: types::Handler::File("index.html".to_string()),
                                        middlewares: vec![
                                            types::Middleware::Gzip,
                                            types::Middleware::Log,
                                            types::Middleware::Auth {
                                                username: "admin".to_string(),
                                                password: "password123".to_string(),
                                            },
                                            types::Middleware::Cache("30s".to_string()),
                                        ],
                                    },
                                    types::Route {
                                        path: "/api/**".to_string(),
                                        handler: types::Handler::Proxy(
                                            types::LoadBalancer::NoBalancer(
                                                Upstream::new("http://localhost:3000".to_string())
                                                    .unwrap()
                                            )
                                        ),
                                        middlewares: vec![
                                            types::Middleware::Cors,
                                            types::Middleware::RateLimit(10),
                                        ],
                                    },
                                    types::Route {
                                        path: "/static-response".to_string(),
                                        handler: types::Handler::Respond {
                                            status: None,
                                            body: Some("Hello, world!".to_string()),
                                        },
                                        middlewares: vec![],
                                    },
                                    types::Route {
                                        path: "/health".to_string(),
                                        handler: types::Handler::Respond {
                                            status: Some(200),
                                            body: None,
                                        },
                                        middlewares: vec![],
                                    },
                                    types::Route {
                                        path: "/secret".to_string(),
                                        handler: types::Handler::Respond {
                                            status: Some(403),
                                            body: Some("Access Denied".to_string()),
                                        },
                                        middlewares: vec![],
                                    },
                                    types::Route {
                                        path: "/old-path".to_string(),
                                        handler: types::Handler::Redirect {
                                            status_code: None,
                                            path: Some("/new-path".to_string()),
                                        },
                                        middlewares: vec![],
                                    },
                                    types::Route {
                                        path: "/old-path-with-status".to_string(),
                                        handler: types::Handler::Redirect {
                                            status_code: Some(301),
                                            path: Some("/new-path".to_string()),
                                        },
                                        middlewares: vec![],
                                    },
                                    types::Route {
                                        path: "/example".to_string(),
                                        handler: types::Handler::Respond {
                                            status: Some(200),
                                            body: Some("<h1>Example</h1>".to_string()),
                                        },
                                        middlewares: vec![
                                            types::Middleware::Header {
                                                operator: types::HeaderOperator::Set,
                                                name: "X-Set-Or-Overwrite-Example-Header"
                                                    .to_string(),
                                                value: Some("value".to_string()),
                                                replace_with: None,
                                            },
                                            types::Middleware::Header {
                                                operator: types::HeaderOperator::DeferSet,
                                                name: "X-Set-With-defer".to_string(),
                                                value: Some("value".to_string()),
                                                replace_with: None,
                                            },
                                            types::Middleware::Header {
                                                operator: types::HeaderOperator::Delete,
                                                name: "X-Delete-Example-Header".to_string(),
                                                value: None,
                                                replace_with: None,
                                            },
                                            types::Middleware::Header {
                                                operator: types::HeaderOperator::Add,
                                                name: "X-Add-Example-Header".to_string(),
                                                value: Some("value".to_string()),
                                                replace_with: None,
                                            },
                                            types::Middleware::Header {
                                                operator: types::HeaderOperator::Default,
                                                name: "X-Set-If-NotExist-Example-Header"
                                                    .to_string(),
                                                value: Some("value".to_string()),
                                                replace_with: None,
                                            },
                                            types::Middleware::Header {
                                                operator: types::HeaderOperator::Replace,
                                                name: "X-Replace-Header-Value".to_string(),
                                                value: Some("value".to_string()),
                                                replace_with: Some("replace_with_this".to_string()),
                                            },
                                            types::Middleware::Header {
                                                operator: types::HeaderOperator::DeferReplace,
                                                name: "X-Replace-Header-Value-With-Defer"
                                                    .to_string(),
                                                value: Some("value".to_string()),
                                                replace_with: Some("replace_with_this".to_string()),
                                            },
                                        ],
                                    },
                                ],
                            },
                            types::VirtualHost {
                                domain: "example.com".to_string(),
                                routes: vec![
                                    types::Route {
                                        path: "/blog/**".to_string(),
                                        handler: types::Handler::Proxy(
                                            types::LoadBalancer::NoBalancer(
                                                Upstream::new(
                                                    "http://blog.example.com".to_string()
                                                )
                                                .unwrap()
                                            )
                                        ),
                                        middlewares: vec![
                                            types::Middleware::Gzip,
                                            types::Middleware::Cache("5m".to_string()),
                                        ],
                                    },
                                    types::Route {
                                        path: "/admin".to_string(),
                                        handler: types::Handler::Proxy(
                                            types::LoadBalancer::NoBalancer(
                                                Upstream::new(
                                                    "http://admin.example.com".to_string()
                                                )
                                                .unwrap()
                                            )
                                        ),
                                        middlewares: vec![types::Middleware::Auth {
                                            username: "superuser".to_string(),
                                            password: "secret".to_string(),
                                        },],
                                    },
                                ],
                            },
                        ]
                    }
                ))
            );
        }
    }
}
