// Manual test to validate the timeout configuration functionality

fn main() {
    // Test 1: Basic proxy parsing (should work)
    println!("=== Test 1: Basic proxy parsing ===");
    let simple_config = "localhost { route /test/* { proxy http://localhost:8080 } }";
    
    match chico_file::parse_config(simple_config) {
        Ok((remaining, config)) => {
            println!("✓ Simple proxy config parsed successfully!");
            println!("  Remaining: '{}'", remaining);
            
            let route = &config.virtual_hosts[0].routes[0];
            match &route.handler {
                chico_file::types::Handler::Proxy(proxy_config) => {
                    println!("  Request timeout: {:?}", proxy_config.request_timeout);
                    println!("  Connection timeout: {:?}", proxy_config.connection_timeout);
                    println!("  ✓ Simple proxy test passed");
                }
                _ => println!("  ✗ Not a proxy handler"),
            }
        }
        Err(e) => {
            println!("  ✗ Parse error: {}", e);
        }
    }
    
    // Test 2: Proxy with timeouts
    println!("\n=== Test 2: Proxy with timeouts ===");
    let timeout_config = "localhost { route /api/* { proxy { upstreams http://localhost:8080 request_timeout 25 connection_timeout 10 } } }";
    
    match chico_file::parse_config(timeout_config) {
        Ok((remaining, config)) => {
            println!("✓ Timeout proxy config parsed successfully!");
            println!("  Remaining: '{}'", remaining);
            
            let route = &config.virtual_hosts[0].routes[0];
            match &route.handler {
                chico_file::types::Handler::Proxy(proxy_config) => {
                    println!("  Request timeout: {:?}", proxy_config.request_timeout);
                    println!("  Connection timeout: {:?}", proxy_config.connection_timeout);
                    
                    if proxy_config.request_timeout == Some(25) && proxy_config.connection_timeout == Some(10) {
                        println!("  ✓ Timeout values parsed correctly!");
                    } else {
                        println!("  ✗ Timeout values incorrect");
                    }
                }
                _ => println!("  ✗ Not a proxy handler"),
            }
        }
        Err(e) => {
            println!("  ✗ Parse error: {}", e);
        }
    }
    
    // Test 3: Proxy with only request timeout
    println!("\n=== Test 3: Proxy with partial timeouts ===");
    let partial_config = "localhost { route /partial/* { proxy { upstreams http://localhost:8080 request_timeout 15 } } }";
    
    match chico_file::parse_config(partial_config) {
        Ok((remaining, config)) => {
            println!("✓ Partial timeout config parsed successfully!");
            println!("  Remaining: '{}'", remaining);
            
            let route = &config.virtual_hosts[0].routes[0];
            match &route.handler {
                chico_file::types::Handler::Proxy(proxy_config) => {
                    println!("  Request timeout: {:?}", proxy_config.request_timeout);
                    println!("  Connection timeout: {:?}", proxy_config.connection_timeout);
                    
                    if proxy_config.request_timeout == Some(15) && proxy_config.connection_timeout.is_none() {
                        println!("  ✓ Partial timeout configuration correct!");
                    } else {
                        println!("  ✗ Partial timeout values incorrect");
                    }
                }
                _ => println!("  ✗ Not a proxy handler"),
            }
        }
        Err(e) => {
            println!("  ✗ Parse error: {}", e);
        }
    }
    
    println!("\n🎉 Manual timeout configuration tests complete!");
}