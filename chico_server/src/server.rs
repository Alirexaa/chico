use chico_file::types::Config;
use http::{Request, Response};
use hyper::{body::Body, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use log::{error, info};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

use crate::{
    config::ConfigExt,
    handlers::{select_handler, BoxBody, RequestHandler},
};

pub async fn run_server(config: Config) {
    let ports = config.get_ports();

    let socket_addresses = ports
        .into_iter()
        .map(|port| SocketAddr::from(([127, 0, 0, 1], port)));

    let mut listeners = vec![];

    for addr in socket_addresses {
        let listener = match TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(e) => {
                error!("Failed to bind to address {}: {:?}", addr, e);
                return;
            }
        };
        listeners.push(listener);

        // We wait for following text to be written in standard output (stdout) in integration tests.
        // Any change at this message should be applied in tests.
        info!(
            "Start listening to incoming requests on port {}",
            &addr.port()
        );
    }

    let mut handles = vec![];

    let config = Arc::new(config);

    for listener in listeners {
        let config_clone = config.clone();
        let join_handle =
            tokio::spawn(async move { handle_listener(config_clone, listener).await });
        handles.push(join_handle);
    }

    for handle in handles {
        let _ = handle.await; // Wait for each listener to complete
    }
}

async fn handle_listener(config: Arc<Config>, listener: TcpListener) -> ! {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                error!("Error accepting connection: {:?}", e);
                continue;
            }
        };
        let config_clone = config.clone();

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::spawn(async move {
            _ = handle_connection(config_clone, stream).await;
        });
    }
}

async fn handle_connection(config: Arc<Config>, stream: tokio::net::TcpStream) {
    // Use an adapter to access something implementing `tokio::io` traits as if they implement
    // `hyper::rt` IO traits.
    let io = TokioIo::new(stream);

    let config_clone = config.clone();

    let service = service_fn(move |req| {
        let config_clone = config_clone.clone();
        async move { handle_request(req, config_clone).await }
    });

    if let Err(err) = http1::Builder::new()
        // `service_fn` converts our function in a `Service`
        .serve_connection(io, service)
        .await
    {
        error!("Error serving connection: {:?}", err);
    }
}

async fn handle_request(
    request: Request<impl Body>,
    config: Arc<Config>,
) -> Result<Response<BoxBody>, Infallible> {
    Ok(select_handler(&request, config).handle(request).await)
}
