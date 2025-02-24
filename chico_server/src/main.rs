#![cfg_attr(feature = "strict", deny(warnings))]

use chico_file::parse_config;
fn main() {
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

        route /example {
            respond "<h1>Example</h1>" 200
            
            #header Content-Type text/html
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

    match parse_config(input) {
        Ok((_, config)) => println!("{:#?}", config),
        Err(err) => eprintln!("Parsing failed: {:?}", err),
    }
}
