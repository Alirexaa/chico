#![cfg_attr(feature = "strict", deny(warnings))]

use chico_file::parse_config;
fn main() {
    let input = r#"
    localhost {
        route / {
            file index.html
            gzip
            log 
            auth admin password123
            cache 30s
        }

        route /api/** {
            proxy http://localhost:3000
            cors
            rate_limit 10 
        }
    }
    
    example.com {
        route /blog/** {
            proxy http://blog.example.com
            gzip
            cache 5m
        }
        
        route /admin {
            proxy http://admin.example.com
            auth superuser secret
        }
    }
"#;

    match parse_config(input) {
        Ok((_, config)) => println!("{:#?}", config),
        Err(err) => eprintln!("Parsing failed: {:?}", err),
    }
}
