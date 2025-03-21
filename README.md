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
    route /api/* {
        proxy http://localhost:3000
        cors
        rate_limit 10
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

### Testing

To run the tests, use the following command:

```sh
cargo test
```

### License

This project is licensed under the Apache License 2.0

### Development Status

This project is under active development and is not ready for production use at this time.
