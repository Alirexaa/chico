# Chico

[![CI](https://github.com/Alirexaa/chico/actions/workflows/ci.yaml/badge.svg)](https://github.com/Alirexaa/chico/actions/workflows/ci.yaml) 
[![codecov](https://codecov.io/github/Alirexaa/chico/graph/badge.svg?token=HLA0G8M60V)](https://codecov.io/github/Alirexaa/chico)
![GitHub License](https://img.shields.io/github/license/alirexaa/chico?color=blue)


Chico is a fast web server and load balancer written in Rust. It supports various handlers like file serving, proxying, directory browsing, and custom responses.

## Features

- **File Handler**: Serve static files.
- **Proxy Handler**: Proxy requests to another server.
- **Directory Handler**: Serve directory listings.
- **Respond Handler**: Return custom responses.
- **Redirect Handler**: Redirect requests to another path.
- **Middleware Support**: Add middleware like gzip, cors, logging, rate limiting, etc.

## Getting Started

### Prerequisites

- Rust (version 1.85.0)

### Installation

1. Clone the repository:
    ```sh
    git clone https://github.com/alirexaa/chico.git
    cd chico
    ```

2. Build the project:
    ```sh
    cargo build
    ```

### Running the Server

To run the server, use the following command:
```sh
cargo run --bin chico -- run --config <path_to_config_file>
```

### Validating Configuration

To validate the configuration file, use the following command:

```sh
cargo run --bin chico -- validate --config <path_to_config_file>
```

### Configuration

The configuration file is written in a custom format and supports defining virtual hosts, routes, and handlers. Here is an example configuration:


```
localhost {
    route / {
        file index.html
    }
    
    # Simple proxy (backward compatible)
    route /api/* {
        proxy http://localhost:3000
        cors
        rate_limit 10
    }
    
    # Advanced proxy with load balancing
    route /load-balanced/* {
        proxy {
            upstreams http://backend1:8080 http://backend2:8080 http://backend3:8080
            lb_policy round_robin
        }
        cors
    }
    
    # Single upstream using new syntax
    route /single-backend/* {
        proxy {
            upstreams http://backend:9000
        }
    }
    
    route /static-response {
        respond "Hello, world!"
    }
    route /health {
        respond 200
    }
    route /secret {
        respond "Access Denied" 403
    }
    route /old-path {
        redirect /new-path
    }
    route /old-path-with-status {
        redirect /new-path 301
    }
}
```

#### Proxy Configuration

Chico supports two proxy configuration formats:

**Simple Proxy (Backward Compatible):**
```
route /api/* {
    proxy http://backend:3000
}
```

**Advanced Proxy with Load Balancing:**
```
route /api/* {
    proxy {
        upstreams http://backend1:8080 http://backend2:8080 http://backend3:8080
        lb_policy round_robin
    }
}
```

The `lb_policy` supports:
- Empty value (default): Uses no load balancer for single upstream
- `round_robin`: Distributes requests evenly across multiple upstreams

When multiple upstreams are specified without `lb_policy`, it defaults to `round_robin`.

**Proxy with Timeout Configuration:**
```
route /api/* {
    proxy {
        upstreams http://backend1:8080 http://backend2:8080
        lb_policy round_robin
        request_timeout 30
        connection_timeout 10
    }
}
```

**Timeout Configuration Options:**
- `request_timeout` (seconds): Maximum time to wait for a response from the upstream server (default: 30 seconds)
- `connection_timeout` (seconds): Maximum time to wait when establishing a connection to the upstream server (default: 10 seconds)

Both timeout options are optional and can be configured independently:
```
# Only request timeout
proxy {
    upstreams http://backend:8080
    request_timeout 15
}

# Only connection timeout  
proxy {
    upstreams http://backend:8080
    connection_timeout 5
}

# Both timeouts
proxy {
    upstreams http://backend:8080
    request_timeout 25
    connection_timeout 8
}
```

### Testing

To run the tests, use the following command:

```sh
cargo test --all-features
```

### License

This project is licensed under the Apache License 2.0

### Development Status

This project is under active development and is not ready for production use at this time.
