use chico_file::{parse_config, types::Config};

use crate::virtual_host::VirtualHostExt;

pub trait ConfigExt {
    fn get_ports(&self) -> Vec<u16>;
}

impl ConfigExt for Config {
    fn get_ports(&self) -> Vec<u16> {
        self.virtual_hosts.iter().map(|vh| vh.get_port()).collect()
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
        return Err(format!(
            "Failed to parse config file. reason: {}",
            parse_result.err().unwrap()
        ));
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
}
