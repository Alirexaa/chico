use std::{convert::Infallible, net::SocketAddr};

use chico_file::types::Config;
use http::{Request, Response};
use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

pub async fn run_server(config: Config) {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();

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
    _request: Request<hyper::body::Incoming>,
    _config: Config,
) -> Result<Response<Full<Bytes>>, Infallible> {
    todo!()
}
