use chico_file::types::*;

fn main() {
    // Test basic timeout parsing functionality
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

    println!("Testing config parsing with timeouts...");
    
    match chico_file::parse_config(config_content) {
        Ok(config) => {
            println!("✓ Config parsed successfully!");
            
            let vhost = &config.virtual_hosts[0];
            let route = &vhost.routes[0];
            
            match &route.handler {
                Handler::Proxy(proxy_config) => {
                    println!("✓ Proxy handler found");
                    println!("  Request timeout: {:?}", proxy_config.request_timeout);
                    println!("  Connection timeout: {:?}", proxy_config.connection_timeout);
                    
                    if proxy_config.request_timeout == Some(25) && proxy_config.connection_timeout == Some(10) {
                        println!("✓ Timeouts parsed correctly!");
                    } else {
                        println!("✗ Timeout values are incorrect");
                    }
                }
                _ => {
                    println!("✗ Expected proxy handler");
                }
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }
}