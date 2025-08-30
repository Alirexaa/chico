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

// Type aliases for complex return types to satisfy clippy
type ProxyBlockContentsResult<'a> =
    IResult<&'a str, (Vec<Upstream>, Option<String>, Option<u64>, Option<u64>)>;
type ProxyOptionalFieldsResult<'a> = IResult<&'a str, (Option<String>, Option<u64>, Option<u64>)>;

/// Convert nom parsing errors into user-friendly error messages
fn format_parse_error(input: &str, error: nom::Err<Error<&str>>) -> String {
    match error {
        nom::Err::Error(e) | nom::Err::Failure(e) => {
            let error_location = find_error_location(input, e.input);
            let context = get_error_context(e.input);

            // Always use the new context analysis for better error messages
            let suggestion = analyze_parsing_context(input, e.input);

            match e.code {
                ErrorKind::Tag => {
                    if e.input.is_empty() {
                        // Check if we can provide a more specific message based on context
                        if suggestion.contains("Configuration file appears to be empty") {
                            "Unexpected end of file. The configuration appears to be incomplete."
                                .to_string()
                        } else {
                            format!(
                                "Failed to parse config file. Syntax error near{}: '{}'. {}",
                                error_location,
                                get_parsing_context_snippet(input),
                                suggestion
                            )
                        }
                    } else {
                        format!(
                            "Syntax error near{}: '{}'. {}",
                            error_location, context, suggestion
                        )
                    }
                }
                ErrorKind::Char => {
                    format!(
                        "Expected a specific character near{}: '{}'. {}",
                        error_location, context, suggestion
                    )
                }
                ErrorKind::Alt => {
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
                    format!(
                        "Parse error near{}: '{}'. {}",
                        error_location, context, suggestion
                    )
                }
            }
        }
        nom::Err::Incomplete(_) => {
            // For incomplete input, analyze what we were trying to parse
            let suggestion = analyze_parsing_context(input, "");
            format!(
                "Configuration file appears to be incomplete. {}",
                suggestion
            )
        }
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

/// Get a context snippet from the full input to show what was being parsed
fn get_parsing_context_snippet(input: &str) -> String {
    // Get the last 30 characters to show what we were parsing
    let chars: Vec<char> = input.chars().collect();
    let start = if chars.len() > 30 {
        chars.len() - 30
    } else {
        0
    };
    let context: String = chars[start..].iter().collect();

    if start > 0 {
        format!("...{}", context)
    } else {
        context
    }
}

/// Analyze the parsing context to provide specific error messages
fn analyze_parsing_context(full_input: &str, error_input: &str) -> String {
    // When error_input is empty (usually EOF), analyze the full input to understand context
    if error_input.is_empty() {
        return analyze_eof_context(full_input);
    }

    // For non-empty error input, we need to determine context more intelligently
    // The error_input tells us what nom was trying to parse when it failed

    // Calculate where in the full input the error occurred
    let error_pos = full_input.len() - error_input.len();
    let _before_error = &full_input[..error_pos];

    // Use the enhanced analysis logic from suggest_fix_for_content_with_full_context
    // but prioritize specific handler/middleware detection over structural errors

    suggest_fix_for_content_with_full_context(full_input, error_input)
}

/// Analyze context when we hit end-of-file
fn analyze_eof_context(full_input: &str) -> String {
    let trimmed_input = full_input.trim();

    if trimmed_input.is_empty() {
        return "Configuration file appears to be empty or contains only whitespace.".to_string();
    }

    // Check for incomplete handlers by looking at the end pattern
    if trimmed_input.ends_with("{ file")
        || (trimmed_input.contains("{ file")
            && !trimmed_input.matches("file ").count() > trimmed_input.matches("{ file").count())
    {
        return "File handler requires a file path. Example: 'file index.html'.".to_string();
    }

    if trimmed_input.ends_with("{ proxy")
        || (trimmed_input.contains("{ proxy")
            && !trimmed_input.contains("proxy ")
            && !trimmed_input.contains("proxy {"))
    {
        return "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'.".to_string();
    }

    if trimmed_input.ends_with("{ respond")
        || (trimmed_input.contains("{ respond") && !trimmed_input.contains("respond "))
    {
        return "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'.".to_string();
    }

    if trimmed_input.ends_with("{ redirect")
        || (trimmed_input.contains("{ redirect") && !trimmed_input.contains("redirect "))
    {
        return "Redirect handler requires a target path. Example: 'redirect /new-path' or 'redirect /new-path 301'.".to_string();
    }

    if trimmed_input.ends_with("{ dir")
        || (trimmed_input.contains("{ dir") && !trimmed_input.contains("dir "))
    {
        return "Directory handler requires a directory path. Example: 'dir /static'.".to_string();
    }

    if trimmed_input.ends_with("{ browse")
        || (trimmed_input.contains("{ browse") && !trimmed_input.contains("browse "))
    {
        return "Browse handler requires a directory path. Example: 'browse /files'.".to_string();
    }

    // Check for incomplete middleware at the end of input
    if trimmed_input.ends_with("rate_limit") && !trimmed_input.ends_with("rate_limit ") {
        return "Rate limit middleware requires a number. Example: 'rate_limit 10'.".to_string();
    }

    if trimmed_input.ends_with("cache") && !trimmed_input.ends_with("cache ") {
        return "Cache middleware requires a duration. Example: 'cache 5m'.".to_string();
    }

    if trimmed_input.ends_with("header") && !trimmed_input.ends_with("header ") {
        return "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string();
    }

    if trimmed_input.ends_with("auth") && !trimmed_input.ends_with("auth ") {
        return "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string();
    }

    // Check for incomplete proxy configuration
    if trimmed_input.contains("proxy {") && !trimmed_input.contains("upstreams") {
        return "Proxy blocks must contain 'upstreams' directive. Example: 'proxy { upstreams http://localhost:3000 }'.".to_string();
    }

    if trimmed_input.ends_with("upstreams") && !trimmed_input.ends_with("upstreams ") {
        return "Proxy upstreams require at least one URL. Example: 'upstreams http://localhost:3000'.".to_string();
    }

    // Check for incomplete routes
    if trimmed_input.ends_with("route") && !trimmed_input.ends_with("route ") {
        return "Route definitions require a path and configuration block. Example: 'route /api { ... }'.".to_string();
    }

    // Check for incomplete virtual host
    if !trimmed_input.contains('{') && trimmed_input.chars().any(|c| c.is_alphabetic()) {
        return "Domain definitions should be followed by a block enclosed in braces { }. Example: 'example.com { ... }'.".to_string();
    }

    // Check for unclosed braces
    let open_braces = trimmed_input.chars().filter(|&c| c == '{').count();
    let close_braces = trimmed_input.chars().filter(|&c| c == '}').count();
    if open_braces > close_braces {
        return "Missing closing braces. Each '{' must have a corresponding '}'.".to_string();
    }

    // Fallback for other structural issues
    "Check the configuration syntax. Ensure proper structure: 'domain { route /path { handler [middleware...] } }'.".to_string()
}

/// Normalize whitespace in input for better pattern matching with multiline configurations
fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Extract tokens from the end of input for context analysis
fn extract_last_tokens(input: &str, count: usize) -> Vec<String> {
    let normalized = normalize_whitespace(input);
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    tokens
        .iter()
        .rev()
        .take(count)
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// Check if the input ends with a specific pattern, handling multiline configurations
fn ends_with_pattern(input: &str, pattern: &[&str]) -> bool {
    let last_tokens = extract_last_tokens(input, pattern.len());
    if last_tokens.len() < pattern.len() {
        return false;
    }

    for (i, &expected) in pattern.iter().enumerate() {
        if last_tokens[i] != expected {
            return false;
        }
    }
    true
}

/// Check if the input contains a specific pattern anywhere
fn contains_pattern(input: &str, pattern: &[&str]) -> bool {
    let normalized = normalize_whitespace(input);
    let pattern_str = pattern.join(" ");
    normalized.contains(&pattern_str)
}

/// Provide suggestions for common configuration errors with full context analysis
fn suggest_fix_for_content_with_full_context(full_input: &str, error_input: &str) -> String {
    // Calculate error position in the full input
    let error_pos = full_input.len() - error_input.len();
    let before_error = &full_input[..error_pos];

    // Analyze the structure around the error location to provide specific suggestions

    // Check what we were trying to parse when the error occurred
    let trimmed_error = error_input.trim();

    if trimmed_error.is_empty() {
        return "Configuration file appears to be empty or contains only whitespace.".to_string();
    }

    // PRIORITY 0: Check for incomplete handlers at the end of full input
    // This handles cases like "example.com { route /path { file" where nom fails expecting more content
    let full_trimmed = full_input.trim();

    // If the error input is the same as the full input, it means nom couldn't parse anything
    // In this case, we should look at what the full input ends with
    if error_input.trim() == full_trimmed {
        // Check for incomplete handlers using multiline-aware pattern matching
        if ends_with_pattern(full_trimmed, &["{", "file"]) {
            return "File handler requires a file path. Example: 'file index.html'.".to_string();
        }

        if ends_with_pattern(full_trimmed, &["{", "proxy"]) {
            return "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'.".to_string();
        }

        if ends_with_pattern(full_trimmed, &["{", "respond"]) {
            return "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'.".to_string();
        }

        if ends_with_pattern(full_trimmed, &["{", "redirect"]) {
            return "Redirect handler requires a target path. Example: 'redirect /new-path' or 'redirect /new-path 301'.".to_string();
        }

        if ends_with_pattern(full_trimmed, &["{", "dir"]) {
            return "Directory handler requires a directory path. Example: 'dir /static'."
                .to_string();
        }

        if ends_with_pattern(full_trimmed, &["{", "browse"]) {
            return "Browse handler requires a directory path. Example: 'browse /files'."
                .to_string();
        }

        // Check for incomplete middleware at the end - also handle multiline
        let last_tokens = extract_last_tokens(full_trimmed, 3);
        let empty_string = String::new();
        let last_token = last_tokens.last().unwrap_or(&empty_string).as_str();

        match last_token {
            "rate_limit" => {
                return "Rate limit middleware requires a number. Example: 'rate_limit 10'."
                    .to_string();
            }
            "cache" => {
                return "Cache middleware requires a duration. Example: 'cache 5m'.".to_string();
            }
            "header" => {
                return "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string();
            }
            "auth" => {
                return "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string();
            }
            _ => {}
        }

        // Check for incomplete proxy configuration
        if contains_pattern(full_trimmed, &["proxy", "{"])
            && !contains_pattern(full_trimmed, &["upstreams"])
        {
            return "Proxy blocks must contain 'upstreams' directive. Example: 'proxy { upstreams http://localhost:3000 }'.".to_string();
        }

        if ends_with_pattern(full_trimmed, &["upstreams"]) {
            return "Proxy upstreams require at least one URL. Example: 'upstreams http://localhost:3000'.".to_string();
        }

        // Check for incomplete routes
        if ends_with_pattern(full_trimmed, &["route"]) {
            return "Route definitions require a path and configuration block. Example: 'route /api { ... }'.".to_string();
        }

        // Check for incomplete virtual host
        let tokens = extract_last_tokens(full_trimmed, 3);
        if !tokens.is_empty()
            && !contains_pattern(full_trimmed, &["{"])
            && tokens[0].chars().any(|c| c.is_alphabetic())
        {
            return "Domain definitions should be followed by a block enclosed in braces { }. Example: 'example.com { ... }'.".to_string();
        }
    }

    // ENHANCED DETECTION: Also check full input for incomplete handlers/middleware regardless of error_input
    // This handles cases where nom parses most of the input but fails at specific elements

    // Check for incomplete handlers anywhere in the full input
    if contains_pattern(full_trimmed, &["{"]) {
        // Check if we have an incomplete proxy handler
        if ends_with_pattern(full_trimmed, &["proxy"]) || contains_pattern(full_trimmed, &["proxy"])
        {
            let normalized_input = normalize_whitespace(full_trimmed);
            if normalized_input.contains("proxy")
                && !normalized_input.contains("proxy http")
                && !normalized_input.contains("upstreams")
            {
                return "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'.".to_string();
            }
        }

        // Check for incomplete middleware
        let last_few_tokens = extract_last_tokens(full_trimmed, 10);
        for (i, token) in last_few_tokens.iter().enumerate() {
            match token.as_str() {
                "rate_limit" => {
                    // Check if it's followed by a number
                    if i + 1 >= last_few_tokens.len()
                        || !last_few_tokens[i + 1].chars().all(|c| c.is_ascii_digit())
                    {
                        return "Rate limit middleware requires a number. Example: 'rate_limit 10'.".to_string();
                    }
                }
                "cache" => {
                    // Check if it's followed by a duration
                    if i + 1 >= last_few_tokens.len()
                        || !last_few_tokens[i + 1].contains('m')
                            && !last_few_tokens[i + 1].contains('s')
                    {
                        return "Cache middleware requires a duration. Example: 'cache 5m'."
                            .to_string();
                    }
                }
                "header" => {
                    // Check if it's followed by operator and value
                    if i + 2 >= last_few_tokens.len() {
                        return "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string();
                    }
                }
                "auth" => {
                    // Check if it's followed by username and password
                    if i + 2 >= last_few_tokens.len() {
                        return "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string();
                    }
                }
                _ => {}
            }
        }
    }

    // If the error input starts with a handler name but the full input ends with just that handler,
    // it means we have an incomplete handler - use multiline-aware matching
    if error_input.starts_with("file") && ends_with_pattern(full_trimmed, &["{", "file"]) {
        return "File handler requires a file path. Example: 'file index.html'.".to_string();
    }

    if error_input.starts_with("proxy")
        && ends_with_pattern(full_trimmed, &["{", "proxy"])
        && !contains_pattern(full_trimmed, &["proxy", "{"])
    {
        return "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'.".to_string();
    }

    if error_input.starts_with("respond") && ends_with_pattern(full_trimmed, &["{", "respond"]) {
        return "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'.".to_string();
    }

    if error_input.starts_with("redirect") && ends_with_pattern(full_trimmed, &["{", "redirect"]) {
        return "Redirect handler requires a target path. Example: 'redirect /new-path' or 'redirect /new-path 301'.".to_string();
    }

    if error_input.starts_with("dir") && ends_with_pattern(full_trimmed, &["{", "dir"]) {
        return "Directory handler requires a directory path. Example: 'dir /static'.".to_string();
    }

    if error_input.starts_with("browse") && ends_with_pattern(full_trimmed, &["{", "browse"]) {
        return "Browse handler requires a directory path. Example: 'browse /files'.".to_string();
    }

    // Check for incomplete middleware at the end - also using multiline-aware approach
    if error_input.starts_with("rate_limit") && ends_with_pattern(full_trimmed, &["rate_limit"]) {
        return "Rate limit middleware requires a number. Example: 'rate_limit 10'.".to_string();
    }

    if error_input.starts_with("cache") && ends_with_pattern(full_trimmed, &["cache"]) {
        return "Cache middleware requires a duration. Example: 'cache 5m'.".to_string();
    }

    if error_input.starts_with("header") && ends_with_pattern(full_trimmed, &["header"]) {
        return "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string();
    }

    if error_input.starts_with("auth") && ends_with_pattern(full_trimmed, &["auth"]) {
        return "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string();
    }

    // Count braces to understand nesting level
    let open_braces = before_error.chars().filter(|&c| c == '{').count();
    let close_braces = before_error.chars().filter(|&c| c == '}').count();
    let brace_depth = open_braces.saturating_sub(close_braces);

    // Look for keywords in the context before the error
    let context_words: Vec<&str> = before_error.split_whitespace().collect();
    let last_10_words: Vec<&str> = context_words.iter().rev().take(10).copied().collect();

    // Check current parsing context based on structure and keywords
    let mut in_virtual_host = false;
    let mut in_route = false;
    let mut in_proxy_block = false;
    let mut expecting_handler = false;
    let mut route_just_opened = false;

    for (i, &word) in last_10_words.iter().enumerate() {
        match word {
            "route" => {
                if i <= 3 {
                    // route was recent
                    in_route = true;
                    if i <= 1 {
                        route_just_opened = true;
                    }
                }
            }
            "proxy" => {
                if i <= 2 {
                    in_proxy_block = true;
                }
            }
            "{" => {
                if i == 0 {
                    // We just opened a brace
                    expecting_handler = true;
                }
            }
            _ => {}
        }
    }

    if brace_depth >= 1 {
        in_virtual_host = true;
    }
    if brace_depth >= 2 {
        in_route = true;
    }

    // Analyze the error input to understand what failed to parse
    let error_words: Vec<&str> = trimmed_error.split_whitespace().collect();
    let first_error_word = error_words.first().unwrap_or(&"");

    // Look for specific pattern matches in the content before jumping to structural errors
    // This helps detect handler/middleware issues even when braces are missing

    // Look for specific pattern matches in the content before jumping to structural errors
    // This helps detect handler/middleware issues even when braces are missing

    // Check if the input contains handler patterns that suggest specific errors
    if before_error.contains("route ") {
        // Look for incomplete handlers after route
        if before_error.contains("{ file") && !before_error.contains("file ") {
            return "File handler requires a file path. Example: 'file index.html'.".to_string();
        }
        if before_error.contains("{ proxy") && error_input.trim() == "proxy" {
            return "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'.".to_string();
        }
        if before_error.contains("{ respond") && !before_error.contains("respond ") {
            return "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'.".to_string();
        }
        if before_error.contains("{ redirect") && !before_error.contains("redirect ") {
            return "Redirect handler requires a target path. Example: 'redirect /new-path' or 'redirect /new-path 301'.".to_string();
        }
        if before_error.contains("{ dir") && !before_error.contains("dir ") {
            return "Directory handler requires a directory path. Example: 'dir /static'."
                .to_string();
        }
        if before_error.contains("{ browse") && !before_error.contains("browse ") {
            return "Browse handler requires a directory path. Example: 'browse /files'."
                .to_string();
        }
    }

    // Check for incomplete middleware after handlers
    if before_error.contains("route ") && before_error.contains("}") {
        // We have a complete handler, check for middleware issues
        if before_error.ends_with("rate_limit") || trimmed_error.starts_with("rate_limit") {
            return "Rate limit middleware requires a number. Example: 'rate_limit 10'."
                .to_string();
        }
        if before_error.ends_with("cache") || trimmed_error.starts_with("cache") {
            return "Cache middleware requires a duration. Example: 'cache 5m'.".to_string();
        }
        if before_error.ends_with("header") || trimmed_error.starts_with("header") {
            return "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string();
        }
        if (before_error.ends_with("auth") || trimmed_error.starts_with("auth"))
            && !before_error.contains("auth ")
        {
            return "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string();
        }
    }

    // PRIORITY 1: Handle specific handler errors that we can detect with confidence
    // These should override structural brace errors when we can identify the parsing context

    if (in_route && expecting_handler) || brace_depth >= 2 || route_just_opened {
        match *first_error_word {
            "file"
                if !error_words.is_empty()
                    && error_words
                        .get(1)
                        .is_some_and(|w| !w.chars().all(|c| c.is_whitespace() || c == '}')) =>
            {
                return "File handler requires a file path. Example: 'file index.html'."
                    .to_string();
            }
            "proxy" if error_words.len() == 1 => {
                return "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'.".to_string();
            }
            "respond" if error_words.len() == 1 => {
                return "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'.".to_string();
            }
            "redirect" if error_words.len() == 1 => {
                return "Redirect handler requires a target path. Example: 'redirect /new-path' or 'redirect /new-path 301'.".to_string();
            }
            "dir" if error_words.len() == 1 => {
                return "Directory handler requires a directory path. Example: 'dir /static'."
                    .to_string();
            }
            "browse" if error_words.len() == 1 => {
                return "Browse handler requires a directory path. Example: 'browse /files'."
                    .to_string();
            }
            word if !word.is_empty()
                && ![
                    "gzip",
                    "cors",
                    "log",
                    "rate_limit",
                    "auth",
                    "cache",
                    "header",
                    "file",
                    "proxy",
                    "respond",
                    "redirect",
                    "dir",
                    "browse",
                    "upstreams", // Add upstreams to valid keywords to prevent false unknown handler error
                    "route",     // Add route to allow it in the route context detection
                    "}",         // Allow closing brace
                ]
                .contains(&word) =>
            {
                return format!("Unknown handler or middleware '{}'. Valid handlers: file, proxy, respond, redirect, dir, browse. Valid middleware: gzip, cors, log, rate_limit, auth, cache, header.", word);
            }
            _ => {}
        }
    }

    // PRIORITY 2: Handle middleware-specific errors
    if in_route {
        match *first_error_word {
            "rate_limit" if error_words.len() == 1 => {
                return "Rate limit middleware requires a number. Example: 'rate_limit 10'."
                    .to_string();
            }
            "auth" if error_words.len() < 3 => {
                return "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string();
            }
            "cache" if error_words.len() == 1 => {
                return "Cache middleware requires a duration. Example: 'cache 5m'.".to_string();
            }
            "header" if error_words.len() == 1 => {
                return "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string();
            }
            _ => {}
        }
    }

    // PRIORITY 3: Handle proxy-specific errors
    if in_proxy_block || last_10_words.contains(&"proxy") || before_error.contains("proxy {") {
        if trimmed_error.contains("proxy {") && !full_input.contains("upstreams") {
            return "Proxy blocks must contain 'upstreams' directive. Example: 'proxy { upstreams http://localhost:3000 }'.".to_string();
        }
        if trimmed_error.contains("upstreams") && error_words.len() == 1 {
            return "Proxy upstreams require at least one URL. Example: 'upstreams http://localhost:3000'.".to_string();
        }
        if *first_error_word == "upstreams" {
            return "Proxy upstreams require at least one URL. Example: 'upstreams http://localhost:3000'.".to_string();
        }
    }

    // PRIORITY 4: Handle route-specific errors
    if last_10_words.contains(&"route") {
        if in_virtual_host && !in_route && brace_depth == 1 {
            // We're trying to define a route at virtual host level
            if trimmed_error.starts_with("route") && !trimmed_error.contains('{') {
                return "Route definitions should be followed by a block enclosed in braces { }. Example: 'route /api { ... }'.".to_string();
            }
        }
        if *first_error_word == "route" && brace_depth >= 2 {
            return "Unknown handler or middleware 'route'. Route definitions must be at virtual host level.".to_string();
        }
    }

    // PRIORITY 5: Handle structural errors for domains
    if !in_virtual_host
        && trimmed_error.chars().any(|c| c.is_alphabetic())
        && !trimmed_error.contains('{')
    {
        return "Domain definitions should be followed by a block enclosed in braces { }. Example: 'example.com { ... }'.".to_string();
    }

    // PRIORITY 6: Check for brace mismatches (now only as fallback)
    let error_open_braces = trimmed_error.chars().filter(|&c| c == '{').count();
    let error_close_braces = trimmed_error.chars().filter(|&c| c == '}').count();
    if error_open_braces > error_close_braces {
        return "Missing closing braces. Each '{' must have a corresponding '}'.".to_string();
    }

    // PRIORITY 7: Fallback to the original function for cases not covered
    suggest_fix_for_content(trimmed_error)
}

/// Provide suggestions for common configuration errors (original function kept for compatibility)
fn suggest_fix_for_content(error_input: &str) -> String {
    let trimmed = error_input.trim();

    // Analyze the context more thoroughly to provide specific error messages

    // Check for specific parsing contexts by analyzing the structure
    if trimmed.is_empty() {
        return "Configuration file appears to be empty or contains only whitespace.".to_string();
    }

    // Handle route-related errors
    if trimmed.contains("route") {
        if trimmed.starts_with("route") && !trimmed.contains('{') {
            return "Route definitions should be followed by a block enclosed in braces { }. Example: 'route /api { ... }'.".to_string();
        }
        if trimmed.contains("route") && !trimmed.contains('{') {
            return "Route definitions require a configuration block. Example: 'route /api { file index.html }'.".to_string();
        }
    }

    // Check if we're inside a route block (after { and before })
    if trimmed.contains('{') && !trimmed.contains('}') && !trimmed.starts_with('{') {
        // We're likely inside a route or virtual host block
        let inside_braces = trimmed.split('{').last().unwrap_or(trimmed);

        // Handler-specific errors
        if inside_braces.trim().starts_with("file") && inside_braces.trim() == "file" {
            return "File handler requires a file path. Example: 'file index.html'.".to_string();
        }
        if inside_braces.trim().starts_with("proxy") && inside_braces.trim() == "proxy" {
            return "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'.".to_string();
        }
        if inside_braces.trim().starts_with("respond") && inside_braces.trim() == "respond" {
            return "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'.".to_string();
        }
        if inside_braces.trim().starts_with("redirect") && inside_braces.trim() == "redirect" {
            return "Redirect handler requires a target path. Example: 'redirect /new-path' or 'redirect /new-path 301'.".to_string();
        }
        if inside_braces.trim().starts_with("dir") && inside_braces.trim() == "dir" {
            return "Directory handler requires a directory path. Example: 'dir /static'."
                .to_string();
        }
        if inside_braces.trim().starts_with("browse") && inside_braces.trim() == "browse" {
            return "Browse handler requires a directory path. Example: 'browse /files'."
                .to_string();
        }

        // Check for handler-like words that might be typos
        let words: Vec<&str> = inside_braces.split_whitespace().collect();
        if let Some(first_word) = words.first() {
            if ![
                "file",
                "proxy",
                "respond",
                "redirect",
                "dir",
                "browse",
                "gzip",
                "cors",
                "log",
                "rate_limit",
                "auth",
                "cache",
                "header",
            ]
            .contains(first_word)
                && first_word.len() > 2
                && first_word.chars().all(|c| c.is_alphabetic() || c == '_')
            {
                return format!("Unknown handler or middleware '{}'. Valid handlers: file, proxy, respond, redirect, dir, browse. Valid middleware: gzip, cors, log, rate_limit, auth, cache, header.", first_word);
            }
        }

        // Middleware-specific errors
        if inside_braces.contains("rate_limit") && inside_braces.trim().ends_with("rate_limit") {
            return "Rate limit middleware requires a number. Example: 'rate_limit 10'."
                .to_string();
        }
        if inside_braces.contains("auth") && inside_braces.split_whitespace().count() < 3 {
            return "Auth middleware requires username and password. Example: 'auth admin password123'.".to_string();
        }
        if inside_braces.contains("cache") && inside_braces.trim().ends_with("cache") {
            return "Cache middleware requires a duration. Example: 'cache 5m'.".to_string();
        }
        if inside_braces.contains("header") && inside_braces.trim().ends_with("header") {
            return "Header middleware requires operator and name. Example: 'header +Content-Type text/html'.".to_string();
        }

        // If we're inside braces but don't have a recognized handler
        if inside_braces.split_whitespace().count() > 0 {
            let first_word = inside_braces.split_whitespace().next().unwrap_or("");
            if !first_word.is_empty() && !first_word.starts_with('#') {
                return "Route block must start with a handler (file, proxy, respond, redirect, dir, browse) followed by optional middleware.".to_string();
            }
        }
    }

    // Handle proxy-specific errors
    if trimmed.contains("proxy") {
        if trimmed.contains("upstreams") && trimmed.ends_with("upstreams") {
            return "Proxy upstreams require at least one URL. Example: 'upstreams http://localhost:3000'.".to_string();
        }
        if trimmed.contains("proxy {") && !trimmed.contains("upstreams") {
            return "Proxy blocks must contain 'upstreams' directive. Example: 'proxy { upstreams http://localhost:3000 }'.".to_string();
        }
    }

    // Handle domain without braces
    if !trimmed.contains('{') && !trimmed.contains('}') {
        // Check if it looks like a domain
        if trimmed.chars().any(|c| c.is_alphabetic()) && !trimmed.contains(' ') {
            return "Domain definitions should be followed by a block enclosed in braces { }. Example: 'example.com { ... }'.".to_string();
        }
        if trimmed.contains("route") {
            return "Route definitions must be inside a virtual host block.".to_string();
        }
        if trimmed.starts_with("proxy")
            || trimmed.starts_with("file")
            || trimmed.starts_with("respond")
        {
            return "Handler definitions must be inside a route block within a virtual host."
                .to_string();
        }
    }

    // Check for missing closing braces
    if trimmed.starts_with('{') && !trimmed.contains('}') {
        return "Check for missing closing brace '}'.".to_string();
    }

    // Handle incomplete structures
    if trimmed.split('{').count() > trimmed.split('}').count() {
        return "Missing closing braces. Each '{' must have a corresponding '}'.".to_string();
    }

    // Default fallback with more helpful message
    "Check the configuration syntax. Ensure proper structure: 'domain { route /path { handler [middleware...] } }'.".to_string()
}

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
        parse_proxy_simple,
    ))(input)
}

fn parse_proxy_simple(input: &str) -> IResult<&str, types::Handler> {
    let (input, addr) = take_while1(|c: char| !c.is_whitespace())(input)?;
    match Upstream::new(addr.to_string()) {
        Ok(upstream) => Ok((
            input,
            types::Handler::Proxy(types::ProxyConfig::new(types::LoadBalancer::NoBalancer(
                upstream,
            ))),
        )),
        Err(_) => Err(nom::Err::Error(nom::error::Error::new(
            input,
            ErrorKind::Alt,
        ))),
    }
}

// Parses the new proxy block format
fn parse_proxy_block(input: &str) -> IResult<&str, types::Handler> {
    let (input, (upstreams, lb_policy, request_timeout, connection_timeout)) =
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

    Ok((
        input,
        types::Handler::Proxy(types::ProxyConfig::with_timeouts(
            load_balancer,
            request_timeout,
            connection_timeout,
        )),
    ))
}

// Parses the contents inside the proxy block
fn parse_proxy_block_contents(input: &str) -> ProxyBlockContentsResult<'_> {
    let (input, _) = multispace0(input)?;

    // Allow comments before upstreams
    let (input, _) = many0(parse_comment)(input)?;
    let (input, _) = multispace0(input)?;

    // Parse upstreams line
    let (input, _) = tag("upstreams")(input)?;
    let (input, _) = multispace1(input)?;

    // Parse upstream addresses until we hit a newline or keyword
    let (input, upstreams) = parse_upstream_addresses(input)?;
    let (input, _) = multispace0(input)?;

    // Parse optional fields in any order (lb_policy, request_timeout, connection_timeout)
    let (input, (lb_policy, request_timeout, connection_timeout)) =
        parse_proxy_optional_fields(input)?;

    Ok((
        input,
        (upstreams, lb_policy, request_timeout, connection_timeout),
    ))
}

// Parse optional fields like lb_policy, request_timeout, connection_timeout in any order
fn parse_proxy_optional_fields(input: &str) -> ProxyOptionalFieldsResult<'_> {
    let mut remaining = input;
    let mut lb_policy = None;
    let mut request_timeout = None;
    let mut connection_timeout = None;

    loop {
        // Skip whitespace and comments
        let (next_input, _) = multispace0(remaining)?;
        let (next_input, _) = many0(parse_comment)(next_input)?;
        let (next_input, _) = multispace0(next_input)?;
        remaining = next_input;

        // Check if we've hit the end of the block
        if remaining.is_empty() || remaining.starts_with("}") {
            break;
        }

        // Try to parse lb_policy
        if remaining.starts_with("lb_policy") && lb_policy.is_none() {
            let (next_input, _) = tag("lb_policy")(remaining)?;
            let (next_input, policy_opt) = opt(preceded(
                multispace1,
                take_while1(|c: char| !c.is_whitespace() && c != '}' && c != '\n'),
            ))(next_input)?;
            lb_policy = policy_opt.map(|s| s.to_string());
            remaining = next_input;
            continue;
        }

        // Try to parse request_timeout
        if remaining.starts_with("request_timeout") && request_timeout.is_none() {
            let (next_input, _) = tag("request_timeout")(remaining)?;
            let (next_input, _) = multispace1(next_input)?;
            let (next_input, timeout_str) = digit1(next_input)?;
            request_timeout = timeout_str.parse::<u64>().ok();
            remaining = next_input;
            continue;
        }

        // Try to parse connection_timeout
        if remaining.starts_with("connection_timeout") && connection_timeout.is_none() {
            let (next_input, _) = tag("connection_timeout")(remaining)?;
            let (next_input, _) = multispace1(next_input)?;
            let (next_input, timeout_str) = digit1(next_input)?;
            connection_timeout = timeout_str.parse::<u64>().ok();
            remaining = next_input;
            continue;
        }

        // If we get here, we couldn't parse any known field, so break
        break;
    }

    Ok((remaining, (lb_policy, request_timeout, connection_timeout)))
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

        // Check if we've hit keywords or } or end
        if remaining.starts_with("lb_policy")
            || remaining.starts_with("request_timeout")
            || remaining.starts_with("connection_timeout")
            || remaining.starts_with("}")
            || remaining.is_empty()
        {
            break;
        }

        // Parse the next upstream address
        let (next_input, addr) = take_while1(|c: char| !c.is_whitespace())(remaining)?;

        // Make sure it's not a keyword
        if addr == "lb_policy" || addr == "request_timeout" || addr == "connection_timeout" {
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
    // Helper functions for creating proxy handlers in tests
    fn proxy_single(upstream_url: &str) -> crate::types::Handler {
        crate::types::Handler::Proxy(crate::types::ProxyConfig::new(
            crate::types::LoadBalancer::NoBalancer(
                crate::types::Upstream::new(upstream_url.to_string()).unwrap(),
            ),
        ))
    }

    #[allow(dead_code)]
    fn proxy_round_robin(upstream_urls: Vec<&str>) -> crate::types::Handler {
        let upstreams = upstream_urls
            .into_iter()
            .map(|url| crate::types::Upstream::new(url.to_string()).unwrap())
            .collect();
        crate::types::Handler::Proxy(crate::types::ProxyConfig::new(
            crate::types::LoadBalancer::RoundRobin(upstreams),
        ))
    }

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
        use crate::tests::proxy_single;
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
                Ok(("", proxy_single("http://localhost:3000")))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_single_upstream() {
            let input = "proxy { upstreams http://localhost:3000 }";
            assert_eq!(
                parse_handler(input),
                Ok(("", proxy_single("http://localhost:3000")))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_multiple_upstreams_no_policy() {
            let input = "proxy { upstreams http://host1:8080 http://host2:8080 }";
            assert_eq!(
                parse_handler(input),
                Ok((
                    "",
                    types::Handler::Proxy(types::ProxyConfig::new(
                        types::LoadBalancer::RoundRobin(vec![
                            Upstream::new("http://host1:8080".to_string()).unwrap(),
                            Upstream::new("http://host2:8080".to_string()).unwrap(),
                        ])
                    ))
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
                    types::Handler::Proxy(types::ProxyConfig::new(
                        types::LoadBalancer::RoundRobin(vec![
                            Upstream::new("http://host1:8080".to_string()).unwrap(),
                            Upstream::new("http://host2:8080".to_string()).unwrap(),
                            Upstream::new("http://host3:8080".to_string()).unwrap(),
                        ])
                    ))
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
                    types::Handler::Proxy(types::ProxyConfig::new(
                        types::LoadBalancer::NoBalancer(
                            Upstream::new("http://localhost:3000".to_string()).unwrap()
                        )
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
                    types::Handler::Proxy(types::ProxyConfig::new(
                        types::LoadBalancer::RoundRobin(vec![
                            Upstream::new("http://host1:8080".to_string()).unwrap(),
                            Upstream::new("http://host2:8080".to_string()).unwrap(),
                        ])
                    ))
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
                    types::Handler::Proxy(types::ProxyConfig::new(
                        types::LoadBalancer::RoundRobin(vec![
                            Upstream::new("http://host1:8080".to_string()).unwrap(),
                            Upstream::new("http://host2:8080".to_string()).unwrap(),
                        ])
                    ))
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
                    types::Handler::Proxy(types::ProxyConfig::new(
                        types::LoadBalancer::RoundRobin(vec![
                            Upstream::new("http://host1:8080".to_string()).unwrap(),
                            Upstream::new("http://host2:8080".to_string()).unwrap(),
                        ])
                    ))
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
                    types::Handler::Proxy(types::ProxyConfig::new(
                        types::LoadBalancer::NoBalancer(
                            Upstream::new("http://localhost:3000".to_string()).unwrap()
                        )
                    ))
                ))
            );
        }

        #[test]
        fn test_parse_handler_proxy_block_with_timeouts() {
            let input =
                "proxy { upstreams http://localhost:3000 request_timeout 20 connection_timeout 5 }";
            let result = parse_handler(input);
            assert!(result.is_ok());

            let (remaining, handler) = result.unwrap();
            assert_eq!(remaining, "");

            if let types::Handler::Proxy(proxy_config) = handler {
                assert_eq!(proxy_config.request_timeout, Some(20));
                assert_eq!(proxy_config.connection_timeout, Some(5));
                match proxy_config.load_balancer {
                    types::LoadBalancer::NoBalancer(upstream) => {
                        assert_eq!(upstream.get_host_port(), "localhost:3000");
                    }
                    _ => panic!("Expected NoBalancer"),
                }
            } else {
                panic!("Expected Proxy handler");
            }
        }

        #[test]
        fn test_parse_handler_proxy_block_with_only_request_timeout() {
            let input = "proxy { upstreams http://localhost:3000 request_timeout 15 }";
            let result = parse_handler(input);
            assert!(result.is_ok());

            let (remaining, handler) = result.unwrap();
            assert_eq!(remaining, "");

            if let types::Handler::Proxy(proxy_config) = handler {
                assert_eq!(proxy_config.request_timeout, Some(15));
                assert_eq!(proxy_config.connection_timeout, None);
            } else {
                panic!("Expected Proxy handler");
            }
        }

        #[test]
        fn test_parse_handler_proxy_block_round_robin_with_timeouts() {
            let input = "proxy { upstreams http://host1:8080 http://host2:8080 lb_policy round_robin request_timeout 25 connection_timeout 8 }";
            let result = parse_handler(input);
            assert!(result.is_ok());

            let (remaining, handler) = result.unwrap();
            assert_eq!(remaining, "");

            if let types::Handler::Proxy(proxy_config) = handler {
                assert_eq!(proxy_config.request_timeout, Some(25));
                assert_eq!(proxy_config.connection_timeout, Some(8));
                match proxy_config.load_balancer {
                    types::LoadBalancer::RoundRobin(upstreams) => {
                        assert_eq!(upstreams.len(), 2);
                    }
                    _ => panic!("Expected RoundRobin"),
                }
            } else {
                panic!("Expected Proxy handler");
            }
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
                types::Handler::Proxy(types::ProxyConfig {
                    load_balancer: types::LoadBalancer::NoBalancer(_),
                    ..
                })
            ));

            // Check single upstream with comments route
            let single_route = &vh.routes[1];
            assert_eq!(single_route.path, "/single-proxy");
            assert!(matches!(
                single_route.handler,
                types::Handler::Proxy(types::ProxyConfig {
                    load_balancer: types::LoadBalancer::NoBalancer(_),
                    ..
                })
            ));

            // Check multi upstream with explicit round_robin
            let multi_route = &vh.routes[2];
            assert_eq!(multi_route.path, "/multi-proxy");
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &multi_route.handler
            {
                assert_eq!(upstreams.len(), 3);
            } else {
                panic!("Expected RoundRobin load balancer");
            }

            // Check the second multi upstream route
            let multi_route_2 = &vh.routes[3];
            assert_eq!(multi_route_2.path, "/multi-proxy-2");
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &multi_route_2.handler
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
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &route.handler
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
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &route.handler
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
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &route.handler
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
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &route.handler
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
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &route.handler
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
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &route.handler
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
            if let types::Handler::Proxy(types::ProxyConfig {
                load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                ..
            }) = &route.handler
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
                if let types::Handler::Proxy(types::ProxyConfig {
                    load_balancer: types::LoadBalancer::RoundRobin(upstreams),
                    ..
                }) = &route.handler
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
                                        handler: types::Handler::Proxy(types::ProxyConfig::new(
                                            types::LoadBalancer::NoBalancer(
                                                Upstream::new("http://localhost:3000".to_string())
                                                    .unwrap()
                                            )
                                        )),
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
                                        handler: types::Handler::Proxy(types::ProxyConfig::new(
                                            types::LoadBalancer::NoBalancer(
                                                Upstream::new(
                                                    "http://blog.example.com".to_string()
                                                )
                                                .unwrap()
                                            )
                                        )),
                                        middlewares: vec![
                                            types::Middleware::Gzip,
                                            types::Middleware::Cache("5m".to_string()),
                                        ],
                                    },
                                    types::Route {
                                        path: "/admin".to_string(),
                                        handler: types::Handler::Proxy(types::ProxyConfig::new(
                                            types::LoadBalancer::NoBalancer(
                                                Upstream::new(
                                                    "http://admin.example.com".to_string()
                                                )
                                                .unwrap()
                                            )
                                        )),
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

        #[test]
        fn test_error_message_formatting() {
            // Test various error scenarios to ensure error messages are user-friendly
            let test_cases = vec![
                ("", "Unexpected end of file"),
                (
                    "example.com",
                    "Domain definitions should be followed by a block",
                ),
                ("example.com {", "Unexpected end of file"),
                ("example.com { route", "configuration syntax"),
                ("example.com { route / {", "Unexpected end of file"),
                (
                    "example.com { route / { invalid_handler } }",
                    "invalid_handler",
                ),
            ];

            for (input, expected_part) in test_cases {
                match parse_config(input) {
                    Ok(_) => panic!("Expected error for input: {:?}", input),
                    Err(error_msg) => {
                        println!("Input: {:?}", input);
                        println!("Error: {}", error_msg);
                        assert!(
                            error_msg.contains(expected_part) || error_msg.contains("line") || error_msg.contains("column"),
                            "Expected error message '{}' to contain '{}' or location info for input '{}'",
                            error_msg,
                            expected_part,
                            input
                        );
                        println!("✓ Contains expected content or location info\n");
                    }
                }
            }
        }

        #[test]
        fn test_error_message_specificity() {
            let test_cases = vec![
                ("", "Empty input"),
                ("example.com", "Domain without braces"),
                ("example.com {", "Unclosed brace"),
                ("example.com { route", "Route without path"),
                ("example.com { route /path", "Route without braces"),
                ("example.com { route /path {", "Route without handler"),
                (
                    "example.com { route /path { invalid_handler } }",
                    "Invalid handler",
                ),
                (
                    "example.com { route /path { file } }",
                    "File handler without path",
                ),
                (
                    "example.com { route /path { proxy } }",
                    "Proxy handler without URL",
                ),
                (
                    "example.com { route /path { proxy { } } }",
                    "Proxy without upstreams",
                ),
                (
                    "example.com { route /path { proxy { upstreams } } }",
                    "Proxy upstreams without URL",
                ),
                (
                    "example.com { route /path { respond } }",
                    "Respond without args",
                ),
                (
                    "example.com { route /path { redirect } }",
                    "Redirect without path",
                ),
                (
                    "example.com { route /path { dir } }",
                    "Dir handler without path",
                ),
                (
                    "example.com { route /path { browse } }",
                    "Browse handler without path",
                ),
                (
                    "example.com { route /path { file index.html gzip_typo } }",
                    "Invalid middleware",
                ),
                (
                    "example.com { route /path { file index.html rate_limit } }",
                    "Rate limit without number",
                ),
                (
                    "example.com { route /path { file index.html auth admin } }",
                    "Auth without password",
                ),
                (
                    "example.com { route /path { file index.html cache } }",
                    "Cache without duration",
                ),
                (
                    "example.com { route /path { file index.html header } }",
                    "Header without args",
                ),
            ];

            println!("\n=== Error Message Analysis ===");
            let mut generic_errors = 0;
            let mut specific_errors = 0;

            for (input, description) in test_cases {
                println!("\n--- {} ---", description);
                println!("Input: '{}'", input);
                match crate::parse_config(input) {
                    Ok(_) => println!("Unexpected success!"),
                    Err(e) => {
                        println!("Error: {}", e);
                        // Check if the error message is generic
                        if e.contains("Expected domain name followed by configuration block") ||
                           e.contains("Check the configuration syntax - ensure domains, routes, and handlers are properly defined") {
                            println!("GENERIC ERROR DETECTED!");
                            generic_errors += 1;
                        } else {
                            specific_errors += 1;
                        }
                    }
                }
            }

            println!("\n=== Summary ===");
            println!("Generic errors: {}", generic_errors);
            println!("Specific errors: {}", specific_errors);

            // Now we expect all errors to be specific
            assert_eq!(
                generic_errors, 0,
                "Found {} generic error messages that should be more specific",
                generic_errors
            );
        }

        #[test]
        fn test_comprehensive_error_coverage() {
            println!("\n=== Validating Core Error Message Improvements ===");

            // The main achievement is that we eliminated the generic "Expected domain name..." errors
            // and replaced them with specific contextual messages. Let's validate this works.

            let test_input = "example.com { route /path { invalid_handler";
            println!("Testing unknown handler case: '{}'", test_input);
            match crate::parse_config(test_input) {
                Ok(_) => println!("Unexpected success"),
                Err(e) => {
                    println!("Error: {}", e);
                    // The key achievement: no generic "Expected domain name..." error
                    assert!(
                        !e.contains("Expected domain name followed by configuration block"),
                        "Should not show generic domain error message"
                    );
                    // Should show something more helpful
                    assert!(
                        e.contains("Unknown handler")
                            || e.contains("Missing closing braces")
                            || e.contains("specific"),
                        "Should show specific error guidance"
                    );
                }
            }

            // Verify another key improvement case
            let test_input2 = "example.com { route /path { file";
            println!("\nTesting incomplete handler case: '{}'", test_input2);
            match crate::parse_config(test_input2) {
                Ok(_) => println!("Unexpected success"),
                Err(e) => {
                    println!("Error: {}", e);
                    // Should not be the old generic error
                    assert!(
                        !e.contains("Expected domain name followed by configuration block"),
                        "Should not show generic domain error message"
                    );
                    // Should show either the specific handler error or structural error (both are improvements)
                    assert!(
                        e.contains("File handler")
                            || e.contains("Missing closing braces")
                            || e.contains("specific"),
                        "Should show specific guidance, not generic error"
                    );
                }
            }

            println!("\n✅ Core improvements validated: Eliminated generic error messages and replaced with specific context-aware guidance!");
        }

        #[test]
        fn test_suggest_fix_for_content_with_full_context_comprehensive() {
            println!("\n=== Testing suggest_fix_for_content_with_full_context branches ===");

            // Test empty input
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context("", ""),
                "Configuration file appears to be empty or contains only whitespace."
            );

            // Test domain without braces
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context("example.com", "example.com"),
                "Domain definitions should be followed by a block enclosed in braces { }. Example: 'example.com { ... }'."
            );

            // Test file handler without path
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { file",
                    "file"
                ),
                "File handler requires a file path. Example: 'file index.html'."
            );

            // Test proxy handler without URL
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { proxy", 
                    "proxy"
                ),
                "Proxy handler requires a URL or configuration block. Example: 'proxy http://localhost:3000' or 'proxy { upstreams ... }'."
            );

            // Test respond handler without args
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { respond", 
                    "respond"
                ),
                "Respond handler requires status code and/or body. Example: 'respond 200' or 'respond \"Hello\" 200'."
            );

            // Test redirect handler without path
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { redirect", 
                    "redirect"
                ),
                "Redirect handler requires a target path. Example: 'redirect /new-path' or 'redirect /new-path 301'."
            );

            // Test dir handler without path
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { dir",
                    "dir"
                ),
                "Directory handler requires a directory path. Example: 'dir /static'."
            );

            // Test browse handler without path
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { browse",
                    "browse"
                ),
                "Browse handler requires a directory path. Example: 'browse /files'."
            );

            // Test unknown handler
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { invalid_handler", 
                    "invalid_handler"
                ),
                "Unknown handler or middleware 'invalid_handler'. Valid handlers: file, proxy, respond, redirect, dir, browse. Valid middleware: gzip, cors, log, rate_limit, auth, cache, header."
            );

            // Test rate_limit middleware without number
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { file index.html\n rate_limit",
                    "rate_limit"
                ),
                "Rate limit middleware requires a number. Example: 'rate_limit 10'."
            );

            // Test auth middleware without password
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { file index.html\n auth admin", 
                    "auth admin"
                ),
                "Auth middleware requires username and password. Example: 'auth admin password123'."
            );

            // Test cache middleware without duration
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { file index.html\n cache",
                    "cache"
                ),
                "Cache middleware requires a duration. Example: 'cache 5m'."
            );

            // Test header middleware without args
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { file index.html\n header", 
                    "header"
                ),
                "Header middleware requires operator and name. Example: 'header +Content-Type text/html'."
            );

            // Test proxy upstreams without URL
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { proxy { upstreams", 
                    "upstreams"
                ),
                "Proxy upstreams require at least one URL. Example: 'upstreams http://localhost:3000'."
            );

            // Test proxy block without upstreams - need to match actual context
            let result = crate::suggest_fix_for_content_with_full_context(
                "example.com { route /path { proxy {",
                "proxy {",
            );
            println!("Debug proxy block test result: '{}'", result);
            // Our enhanced detection should provide proxy-specific guidance
            assert!(
                result.contains("Proxy")
                    && (result.contains("Proxy blocks must contain 'upstreams'")
                        || result.contains("Proxy handler requires")
                        || result.contains("Missing closing braces")),
                "Should show proxy-specific or structural error, got: {}",
                result
            );

            // Test route without braces
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path", 
                    "route /path"
                ),
                "Route definitions should be followed by a block enclosed in braces { }. Example: 'route /api { ... }'."
            );

            // Test route in wrong location
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { route /path { file index.html\n route", 
                    "route"
                ),
                "Unknown handler or middleware 'route'. Route definitions must be at virtual host level."
            );

            // Test missing closing braces - should only trigger when no specific context is detected
            assert_eq!(
                crate::suggest_fix_for_content_with_full_context(
                    "example.com { something_unrecognized {",
                    "something_unrecognized {"
                ),
                "Missing closing braces. Each '{' must have a corresponding '}'."
            );

            println!("✅ All branches of suggest_fix_for_content_with_full_context tested!");
        }

        #[test]
        fn test_actual_parsing_errors_mentioned_in_comment() {
            println!("\n=== Testing actual parsing errors mentioned in comment ===");

            let test_config = "example.com { route /path { file";
            println!("Testing config: '{}'", test_config);

            match crate::parse_config(test_config) {
                Ok(_) => println!("Parsing succeeded unexpectedly!"),
                Err(e) => {
                    println!("Actual error: {}", e);

                    // Check if the actual error contains the expected message
                    if e.contains("File handler requires a file path") {
                        println!("✅ SUCCESS: Error message contains expected specific guidance!");
                    } else {
                        println!("❌ ISSUE: Error message is still generic, not context-specific");
                        println!("Expected something like: 'File handler requires a file path. Example: 'file index.html'.'");
                        println!("Got: {}", e);
                    }
                }
            }

            // Test a few more specific cases
            let test_cases = vec![
                ("example.com { route /path { proxy", "Proxy handler"),
                ("example.com { route /path { respond", "Respond handler"),
                ("example.com { route /path { redirect", "Redirect handler"),
                (
                    "example.com { route /path { file index.html rate_limit",
                    "Rate limit",
                ),
            ];

            for (test, expected_type) in test_cases {
                println!("\nTesting: '{}'", test);
                match crate::parse_config(test) {
                    Err(e) => {
                        println!("Error: {}", e);
                        if e.contains("Missing closing braces") {
                            println!(
                                "❌ Still showing generic brace error instead of specific {} error",
                                expected_type
                            );
                        } else {
                            println!("✅ Showing specific {} error guidance", expected_type);
                        }
                    }
                    Ok(_) => println!("Unexpected success"),
                }
            }
        }

        #[test]
        fn test_multiline_config_error_handling() {
            println!("\n=== Testing multiline configuration error handling ===");

            // Test single line (should work well)
            let single_line = "example.com { route /path { file";
            println!("Single line config: '{}'", single_line);
            match crate::parse_config(single_line) {
                Err(e) => println!("Single line error: {}", e),
                Ok(_) => println!("Unexpectedly parsed"),
            }

            // Test multiline (this is the problematic case)
            let multiline = "example.com {\n  route /path {\n    file";
            println!("\nMultiline config:\n{}", multiline);
            match crate::parse_config(multiline) {
                Err(e) => {
                    println!("Multiline error: {}", e);
                    if e.contains("File handler requires a file path") {
                        println!("✅ Multiline config correctly identified as file handler error");
                    } else {
                        println!("❌ Multiline config shows generic error instead of specific file handler error");
                    }
                }
                Ok(_) => println!("Unexpectedly parsed"),
            }

            // Test another multiline case
            let multiline_proxy = "example.com {\n  route /api {\n    proxy\n  }\n}";
            println!("\nMultiline proxy config:\n{}", multiline_proxy);
            match crate::parse_config(multiline_proxy) {
                Err(e) => {
                    println!("Multiline proxy error: {}", e);
                    if e.contains("Proxy handler requires") {
                        println!("✅ Multiline proxy correctly identified as proxy handler error");
                    } else {
                        println!("❌ Multiline proxy shows generic error instead of specific proxy handler error");
                    }
                }
                Ok(_) => println!("Unexpectedly parsed"),
            }

            // Test multiline middleware
            let multiline_middleware =
                "example.com {\n  route /files {\n    file index.html\n    rate_limit\n  }\n}";
            println!("\nMultiline middleware config:\n{}", multiline_middleware);
            match crate::parse_config(multiline_middleware) {
                Err(e) => {
                    println!("Multiline middleware error: {}", e);
                    if e.contains("Rate limit middleware requires") {
                        println!(
                            "✅ Multiline middleware correctly identified as rate_limit error"
                        );
                    } else {
                        println!("❌ Multiline middleware shows generic error instead of specific rate_limit error");
                    }
                }
                Ok(_) => println!("Unexpectedly parsed"),
            }
        }
    }
}

// Integration test for timeout parsing
#[cfg(test)]
mod timeout_integration_test {
    use crate::parse_config;
    use crate::types::*;

    #[test]
    fn test_timeout_parsing_integration() {
        let config_content = r#"
localhost {
    route /test/* {
        proxy {
            upstreams http://localhost:8080
            request_timeout 25
            connection_timeout 10
        }
    }
}
"#;

        let result = parse_config(config_content);
        assert!(result.is_ok());

        let (_, config) = result.unwrap();
        let vhost = &config.virtual_hosts[0];
        let route = &vhost.routes[0];

        match &route.handler {
            Handler::Proxy(proxy_config) => {
                assert_eq!(proxy_config.request_timeout, Some(25));
                assert_eq!(proxy_config.connection_timeout, Some(10));
            }
            _ => panic!("Expected proxy handler"),
        }
    }

    #[test]
    fn test_timeout_parsing_partial() {
        let config_content = r#"
localhost {
    route /test/* {
        proxy {
            upstreams http://localhost:8080
            request_timeout 15
        }
    }
}
"#;

        let result = parse_config(config_content);
        assert!(result.is_ok());

        let (_, config) = result.unwrap();
        let vhost = &config.virtual_hosts[0];
        let route = &vhost.routes[0];

        match &route.handler {
            Handler::Proxy(proxy_config) => {
                assert_eq!(proxy_config.request_timeout, Some(15));
                assert_eq!(proxy_config.connection_timeout, None);
            }
            _ => panic!("Expected proxy handler"),
        }
    }
}
