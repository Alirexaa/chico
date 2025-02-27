#![cfg_attr(feature = "strict", deny(warnings))]

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, digit1, multispace0, none_of, not_line_ending, space1},
    combinator::{map, opt},
    error::{Error, ErrorKind},
    multi::many0,
    sequence::{delimited, preceded, tuple},
    Err, IResult,
};

mod types;

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
        map(preceded(tag("proxy"), parse_value), types::Handler::Proxy),
        map(preceded(tag("dir"), parse_value), types::Handler::Dir),
        map(preceded(tag("browse"), parse_value), types::Handler::Browse),
        map(
            preceded(tag("respond"), parse_respond_handler_args),
            |(status, body)| types::Handler::Respond {
                status: status,
                body: body,
            },
        ),
        map(
            preceded(tag("redirect"), parse_redirect_handler_args),
            |(status_code, path)| types::Handler::Redirect { status_code, path },
        ),
    ))(input)
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
    let (input, num) = take_while1(|c: char| c.is_digit(10))(input)?;
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
pub fn parse_config(input: &str) -> IResult<&str, Vec<types::VirtualHost>> {
    many0(alt((
        map(parse_virtual_host, Some),
        map(parse_comment, |_| None), // Skip comments
    )))(input)
    .map(|(i, hosts)| (i, hosts.into_iter().flatten().collect()))
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

    use crate::{parse_literal_u16, parse_string_u16, parse_u16, string_literal};

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
        use crate::{parse_route, types};

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
    }

    mod handlers {
        use crate::{
            parse_handler, parse_redirect_handler_args, parse_respond_handler_args, types,
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
                    types::Handler::Proxy("http://localhost:3000".to_string())
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
                        operator: operator,
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
}
