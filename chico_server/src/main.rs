#![cfg_attr(feature = "strict", deny(warnings))]
use chico_file::types::{Config, Handler, Route, VirtualHost};
use handlers::select_handler;
use http::{Request, Response};
use http_body_util::Full;
use hyper::{body::Bytes, service::service_fn};
use std::{convert::Infallible, net::SocketAddr, process::exit};

use clap::Parser;
use config::validate_config_file;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
mod cli;
mod config;
mod handlers;
mod virtual_host;
#[tokio::main]
async fn main() {
    let cli = cli::CLI::parse();
    match cli.command {
        cli::Commands::Run => {
            let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

            let listener = TcpListener::bind(addr).await.unwrap();

            loop {
                let (stream, _) = listener.accept().await.unwrap();

                // Use an adapter to access something implementing `tokio::io` traits as if they implement
                // `hyper::rt` IO traits.
                let io = TokioIo::new(stream);

                // Spawn a tokio task to serve multiple connections concurrently
                tokio::task::spawn(async move {
                    // Finally, we bind the incoming connection to our `hello` service
                    if let Err(err) = http1::Builder::new()
                        // `service_fn` converts our function in a `Service`
                        .serve_connection(io, service_fn(handle_request))
                        .await
                    {
                        eprintln!("Error serving connection: {:?}", err);
                    }
                });
            }
        }
        cli::Commands::Validate { config } => {
            validate_config_file(config.as_str())
                .await
                .unwrap_or_else(|err| {
                    eprintln!("{}", err);
                    exit(1);
                });
            println!("✅✅✅ Specified config is valid.");
            exit(0);
        }
    }
}

async fn handle_request(
    request: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let virtual_hosts = vec![VirtualHost {
        domain: "localhost:3000".to_string(),
        routes: vec![Route {
            handler: Handler::Respond {
                status: Some(200),
                body: Some("Hello".to_string()),
            },
            middlewares: vec![],
            path: "/*".to_string(),
        }],
    }];
    let config = Config { virtual_hosts };
    let res = select_handler(&request, config).handle(request);

    Ok(res)
}
