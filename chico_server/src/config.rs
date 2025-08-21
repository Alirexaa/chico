use chico_file::{parse_config, types::Config};
use nom::{error::Error as NomError, error::ErrorKind};

use crate::virtual_host::VirtualHostExt;

pub trait ConfigExt {
    fn get_ports(&self) -> Vec<u16>;
}

impl ConfigExt for Config {
    fn get_ports(&self) -> Vec<u16> {
        self.virtual_hosts.iter().map(|vh| vh.get_port()).collect()
    }
}

/// Convert nom parsing errors into user-friendly error messages
pub(crate) fn format_parse_error(input: &str, error: nom::Err<NomError<&str>>) -> String {
    match error {
        nom::Err::Error(e) | nom::Err::Failure(e) => {
            let error_location = find_error_location(input, e.input);
            let context = get_error_context(e.input);
            
            match e.code {
                ErrorKind::Tag => {
                    if e.input.is_empty() {
                        "Unexpected end of file. The configuration appears to be incomplete.".to_string()
                    } else {
                        let suggestion = suggest_fix_for_content(e.input);
                        format!(
                            "Syntax error near{}: '{}'. {}",
                            error_location,
                            context,
                            suggestion
                        )
                    }
                }
                ErrorKind::Char => {
                    format!(
                        "Expected a specific character near{}: '{}'. Check for missing braces or other syntax elements.",
                        error_location,
                        context
                    )
                }
                ErrorKind::Alt => {
                    let suggestion = suggest_fix_for_content(e.input);
                    format!(
                        "Invalid syntax near{}: '{}'. {}",
                        error_location,
                        context,
                        suggestion
                    )
                }
                ErrorKind::Many1 => {
                    "Expected at least one virtual host definition in the configuration file.".to_string()
                }
                _ => {
                    format!(
                        "Parse error near{}: '{}'. Please check the syntax of your configuration.",
                        error_location,
                        context
                    )
                }
            }
        }
        nom::Err::Incomplete(_) => {
            "Configuration file appears to be incomplete or truncated.".to_string()
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

/// Provide suggestions for common configuration errors
fn suggest_fix_for_content(error_input: &str) -> String {
    let trimmed = error_input.trim();
    
    if trimmed.starts_with('{') && !trimmed.contains('}') {
        "Check for missing closing brace '}'.".to_string()
    } else if trimmed.contains("route") && !trimmed.contains('{') {
        "Route definitions should be followed by a block enclosed in braces { }.".to_string()
    } else if trimmed.chars().any(|c| c.is_alphabetic()) && !trimmed.contains('{') {
        "Domain definitions should be followed by a block enclosed in braces { }.".to_string()
    } else if trimmed.starts_with("proxy") || trimmed.starts_with("file") || trimmed.starts_with("respond") {
        "Handler definitions should be inside a route block.".to_string()
    } else if trimmed.is_empty() {
        "Configuration file appears to be empty or contains only whitespace.".to_string()
    } else {
        "Check the configuration syntax - ensure domains, routes, and handlers are properly defined.".to_string()
    }
}

/// Validate the config file content
pub(crate) async fn validate_config_file(path: &str) -> Result<Config, String> {
    let content = tokio::fs::read_to_string(path).await;
    if content.is_err() {
        return Err(format!(
            "Failed to read the config file. reason: {}",
            content.err().unwrap()
        ));
    }

    let content = content.unwrap();
    parse_with_validate(&content)
}

fn parse_with_validate(content: &str) -> Result<Config, String> {
    if content.is_empty() {
        return Err("Failed to parse content. reason: content is empty.".to_string());
    }

    let parse_result = parse_config(content);

    if parse_result.is_err() {
        let formatted_error = format_parse_error(content, parse_result.err().unwrap());
        return Err(format!("Failed to parse config file. {}", formatted_error));
    }

    let config = parse_result.unwrap().1;
    let virtual_hosts = &config.virtual_hosts;

    if virtual_hosts.is_empty() {
        return Err("Failed to parse config file. reason: no virtual hosts found.".to_string());
    }

    // any logical validation like checking for duplicate domains, routes, etc.

    // checking for duplicate domains
    let mut domains = vec![];
    for host in virtual_hosts.iter() {
        if domains.contains(&host.domain) {
            return Err(format!(
                "Failed to parse config file. reason: duplicate domain found: {}",
                host.domain
            ));
        }
        domains.push(host.domain.clone());
    }

    // checking for duplicate routes
    for host in virtual_hosts.iter() {
        let mut paths = vec![];
        for route in host.routes.iter() {
            if paths.contains(&route.path) {
                return Err(format!(
                    "Failed to parse config file. reason: duplicate in host {} route found: {}",
                    host.domain, route.path
                ));
            }
            paths.push(route.path.clone());
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use chico_file::{
        parse_config,
        types::{Config, Handler, Route, VirtualHost},
    };
    use rstest::rstest;
    use tempfile::NamedTempFile;

    use crate::{
        config::{parse_with_validate, ConfigExt},
        validate_config_file,
    };

    #[test]
    fn test_parse_with_validate_empty_content() {
        let content = "";
        let result = parse_with_validate(content);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Failed to parse content. reason: content is empty."
        );
    }

    #[rstest]
    #[case(
        r#"
        localhost {
            route / {
                file index.html
            }
        }
        localhost {
            route / {
                file index.html
            }
        }
        "#,
        "localhost"
    )]
    #[case(
        r#"
        localhost {
            route / {
                file index.html
            }
        }
        example.com {
            route / {
                file index.html
            }
        }

        example.com {
            route / {
                file index.html
            }
        }

        localhost {
            route / {
                file index.html
            }
        }
        "#,
        "example.com"
    )]
    fn test_parse_with_validate_duplicate_virtual_hosts(
        #[case] content: &str,
        #[case] domain: &str,
    ) {
        let result = parse_with_validate(content);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            format!(
                "Failed to parse config file. reason: duplicate domain found: {}",
                domain
            )
        );
    }

    #[rstest]
    #[case(
        r#"
        localhost {
            route / {
                file index.html
            }
            route /blog {
                file index.html
            }
            route /blog {
                proxy http://localhost:8080
            }
        }
        "#,
        "localhost",
        "/blog"
    )]
    #[case(
        r#"
        localhost {
            route / {
                file index.html
            }
        }
        example.com {
            route / {
                file index.html
            }
            route /api {
                proxy http://localhost:8080
            }
            route /api {
                respond 404
            }
        }
        "#,
        "example.com",
        "/api"
    )]
    fn test_parse_with_validate_duplicate_routes(
        #[case] content: &str,
        #[case] domain: &str,
        #[case] route: &str,
    ) {
        let result = parse_with_validate(content);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            format!(
                "Failed to parse config file. reason: duplicate in host {} route found: {}",
                domain, route
            )
        );
    }

    #[test]
    fn test_parse_with_validate_valid_content() {
        let content = r#"
        localhost {
            route / {
                file index.html
            }
        }
        example.com {
            route / {
                file index.html
            }
        }
        "#;
        let result = parse_with_validate(content);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_file_path_not_exist() {
        let result = validate_config_file("path/to/not/exist").await;
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .contains("Failed to read the config file. reason:"));
    }

    #[tokio::test]
    async fn test_validate_config_file_empty_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let _ = temp_file.write_all(b"");
        let temp_file_path = temp_file.path();
        let temp_dir_path = temp_file_path.to_str().unwrap();
        let result = validate_config_file(temp_dir_path).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Failed to parse content. reason: content is empty."
        );
    }

    #[tokio::test]
    async fn test_validate_config_file_valid_file() {
        let content = r#"
        localhost {
            route / {
                file index.html
            }
        }
        example.com {
            route / {
                file index.html
            }
        }
        "#;

        let mut temp_file = NamedTempFile::new().unwrap();

        let _ = &temp_file.write_all(content.as_bytes());

        let temp_file_path = temp_file.path();
        let temp_file_path = temp_file_path.to_str().unwrap();

        let result = validate_config_file(temp_file_path).await;
        assert_eq!(
            result,
            Ok(Config {
                virtual_hosts: vec![
                    VirtualHost {
                        domain: "localhost".to_string(),
                        routes: vec![Route {
                            path: "/".to_string(),
                            handler: Handler::File("index.html".to_string()),
                            middlewares: vec![],
                        }],
                    },
                    VirtualHost {
                        domain: "example.com".to_string(),
                        routes: vec![Route {
                            path: "/".to_string(),
                            handler: Handler::File("index.html".to_string()),
                            middlewares: vec![],
                        }],
                    }
                ]
            })
        );
    }

    #[tokio::test]
    async fn test_get_ports_when_ports_specified() {
        let content = r#"
        localhost:3000 {
            route / {
                file index.html
            }
        }
        example.com:80 {
            route / {
                file index.html
            }
        }
        http://example2.com:8080 {
            route / {
                file index.html
            }
        }
        https://example3.com:443 {
            route / {
                file index.html
            }
        }
        "#;

        let (_, config) = parse_config(content).unwrap();
        let ports = config.get_ports();
        assert!(ports.contains(&3000));
        assert!(ports.contains(&80));
        assert!(ports.contains(&8080));
        assert!(ports.contains(&443));
    }

    #[tokio::test]
    #[rstest]
    #[case(
        r"
    localhost {
            route / {
                file index.html
            }
        }",
        80
    )]
    #[case(
        r"
    http://example.com {
            route / {
                file index.html
            }
        }",
        80
    )]
    #[case(
        r"
    https://example2.com {
            route / {
                file index.html
            }
        }",
        443
    )]
    async fn test_get_ports_when_ports_not_specified(#[case] content: &str, #[case] port: u16) {
        let (_, config) = parse_config(content).unwrap();
        let ports = config.get_ports();
        assert!(ports.contains(&port));
    }

    #[test]
    fn test_parse_with_validate_improved_error_messages_invalid_syntax() {
        let content = "invalid syntax here";
        let result = parse_with_validate(content);
        assert!(result.is_err());
        let error_msg = result.err().unwrap();
        
        println!("Error message: {}", error_msg);
        
        // Check that error message contains helpful information
        assert!(error_msg.contains("Failed to parse config file"));
        assert!(error_msg.contains("line 1"));
        assert!(error_msg.contains("invalid syntax here"));
    }

    #[test] 
    fn test_parse_with_validate_improved_error_messages_missing_brace() {
        let content = "example.com { route / { file index.html ";
        let result = parse_with_validate(content);
        assert!(result.is_err());
        let error_msg = result.err().unwrap();
        
        // Check that error message suggests missing brace
        assert!(error_msg.contains("Failed to parse config file"));
        assert!(error_msg.contains("line 1"));
    }

    #[test]
    fn test_parse_with_validate_improved_error_messages_multiline() {
        let content = r#"
example.com {
    route / {
        invalid_handler
    }
}
        "#;
        let result = parse_with_validate(content);
        assert!(result.is_err());
        let error_msg = result.err().unwrap();
        
        // Should provide line number information for multiline configs
        assert!(error_msg.contains("Failed to parse config file"));
        assert!(error_msg.contains("line"));
    }

    #[test]
    fn test_format_parse_error_with_suggestions() {
        use chico_file::parse_config;
        
        let test_cases = vec![
            ("", "Unexpected end of file"),
            ("example.com", "Domain definitions should be followed by a block"),
            ("example.com { route", "configuration syntax"),
        ];
        
        for (input, expected_part) in test_cases {
            if let Err(parse_err) = parse_config(input) {
                let formatted = crate::config::format_parse_error(input, parse_err);
                assert!(formatted.contains(expected_part), 
                    "Expected '{}' to contain '{}' for input '{}'", formatted, expected_part, input);
            }
        }
    }
}
