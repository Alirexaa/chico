use chico_file::parse_config;

/// Validate the config file content
pub(crate) async fn validate_config_file(path: &str) -> Result<(), String> {
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

fn parse_with_validate(content: &str) -> Result<(), String> {
    if content.is_empty() {
        return Err(format!(
            "Failed to parse content. reason : content is empty."
        ));
    }

    let parse_result = parse_config(content);

    if parse_result.is_err() {
        return Err(format!(
            "Failed to parse config file. reason: {}",
            parse_result.err().unwrap()
        ));
    }

    let virtual_hosts = parse_result.unwrap().1;

    if virtual_hosts.is_empty() {
        return Err(format!(
            "Failed to parse config file. reason: no virtual hosts found."
        ));
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::{config::parse_with_validate, validate_config_file};

    #[test]
    fn test_parse_with_validate_empty_content() {
        let content = "";
        let result = parse_with_validate(content);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Failed to parse content. reason : content is empty."
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
    async fn test_validate_config_file() {
        let result = validate_config_file("path/to/not/exist").await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Failed to read the config file. reason: The system cannot find the path specified. (os error 3)"
        );
    }
}
