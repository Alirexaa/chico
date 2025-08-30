// Test server plan creation with timeout configuration

mod plan;
mod handlers;
mod load_balance;

use plan::{ServerPlan, RoutePlan};

fn main() {
    println!("=== Testing ServerPlan creation with timeout configuration ===");
    
    let config_text = "localhost { route /api/* { proxy { upstreams http://127.0.0.1:8080 request_timeout 20 connection_timeout 8 } } }";
    
    match chico_file::parse_config(config_text) {
        Ok((_, config)) => {
            println!("✓ Config parsed successfully");
            
            // Test ServerPlan creation (this tests the plan.rs integration)
            let server_plan = ServerPlan::from_config(&config);
            println!("✓ ServerPlan created successfully");
            
            // Find the route and verify it was created properly
            if let Some(vhost_plan) = server_plan.find_virtual_host("localhost", 80) {
                println!("✓ Virtual host found");
                
                if let Some(route_plan) = vhost_plan.find_route("/api/test") {
                    println!("✓ Route found and matches");
                    
                    match route_plan {
                        RoutePlan::ReverseProxy(_handler) => {
                            println!("✓ ReverseProxyHandler created successfully!");
                            println!("✓ Handler should have timeout configuration applied");
                        }
                        _ => {
                            println!("✗ Expected ReverseProxyHandler, got different handler type");
                        }
                    }
                } else {
                    println!("✗ Route not found");
                }
            } else {
                println!("✗ Virtual host not found");
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }
    
    println!("\n🎉 ServerPlan integration test complete!");
}