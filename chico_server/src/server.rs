use chico_file::types::Config;
use http::{Request, Response};
use hyper::{body::Body, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use std::{convert::Infallible, net::SocketAddr};
use tokio::net::TcpListener;

use crate::handlers::{select_handler, BoxBody, RequestHandler};

pub async fn run_server(config: Config) {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind to address {}: {:?}", addr, e);
            return;
        }
    };
    println!("Start listening requests on 3000");

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Error accepting connection: {:?}", e);
                continue;
            }
        };
        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        let config_clone = config.clone();

        let service = service_fn(move |req| {
            let config_clone = config_clone.clone();
            async move { handle_request(req, config_clone).await }
        });

        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service)
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    request: Request<impl Body>,
    config: Config,
) -> Result<Response<BoxBody>, Infallible> {
    Ok(select_handler(&request, config).handle(request).await)
}
