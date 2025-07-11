use chico_file::types::Config;
use http::{Request, Response};
use hyper::body::Incoming;
use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::select;
use tokio::{net::TcpListener, sync::broadcast};
use tracing::{error, info, info_span};

use crate::{
    config::ConfigExt,
    handlers::{self, BoxBody},
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

    // Create a broadcast channel for shutdown signals
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let mut handles = vec![];

    let config = Arc::new(config);

    for listener in listeners {
        let mut rx = shutdown_tx.subscribe();
        let config_clone = config.clone();
        let join_handle =
            tokio::spawn(async move { handle_listener(config_clone, listener, &mut rx).await });
        handles.push(join_handle);
    }

    // Wait for shutdown signal (Ctrl+C)
    shutdown_signal().await;

    info!("Shutdown signal received, notifying listeners...");

    // Send shutdown notification to all listener tasks
    let _ = shutdown_tx.send(());

    // Wait for all listeners to shut down gracefully
    for handle in handles {
        let _ = handle.await; // Wait for each listener to complete
    }
}

async fn handle_listener(
    config: Arc<Config>,
    listener: TcpListener,
    shutdown: &mut broadcast::Receiver<()>,
) {
    loop {
        let span = info_span!("listener.accept.loop");
        let _guard = span.enter();
        select! {
            res = listener.accept() => {
                let (stream, _) = match res {
                    Ok(conn) => conn,
                    Err(e) => {
                        error!("Error accepting connection: {:?}", e);
                        continue;
                    }
                };

                let config_clone = config.clone();

                // Spawn a tokio task to serve multiple connections concurrently
                tokio::spawn(async move {
                    handle_connection(config_clone, stream).await;
                });
            }
            _ = shutdown.recv() => {
                info!("Shutdown signal received, stopping listener");
                break;
            }
        }
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
    request: Request<Incoming>,
    config: Arc<Config>,
) -> Result<Response<BoxBody>, Infallible> {
    let response = handlers::handle_request(request, config).await;
    Ok(response)
}

pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let interrupt = async {
            signal(SignalKind::interrupt())
                .expect("failed to install 'interrupt' signal handler")
                .recv()
                .await;
        };

        let terminate = async {
            signal(SignalKind::terminate())
                .expect("failed to install 'terminate' signal handler")
                .recv()
                .await;
        };

        tokio::select! {
            _ = interrupt => {},
            _ = terminate => {},
        }
    }
    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_c, ctrl_shutdown};
        let interrupt = async {
            ctrl_c()
                .expect("failed to install 'ctrl-c' signal handler")
                .recv()
                .await;
        };

        let terminate = async {
            ctrl_shutdown()
                .expect("failed to install 'ctrl_shutdown' signal handler")
                .recv()
                .await;
        };

        tokio::select! {
            _ = interrupt => {},
            _ = terminate => {},
        }
    }
}
