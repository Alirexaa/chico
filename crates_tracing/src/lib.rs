use std::path::PathBuf;

use directories::ProjectDirs;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initializes the `tracing` logging framework.
///
/// Regular CLI output is influenced by the optional
/// [`RUST_LOG`](tracing_subscriber::filter::EnvFilter) environment variable
/// and is showing all `INFO` level events by default.
pub fn init(log_file_name: String, app_name: String) {
    init_with_default_level(LevelFilter::DEBUG, log_file_name, app_name);
}

fn init_with_default_level(level: LevelFilter, log_file_name: String, app_name: String) {
    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let stdout_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_filter(env_filter)
        .boxed();

    let log_dir = get_log_dir(app_name);
    eprintln!("logs directory {:?}", log_dir);
    let file_appender = tracing_appender::rolling::daily(log_dir, log_file_name);

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Save guard to keep the file open and Prevents drop during runtime
    Box::leak(Box::new(_guard));

    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .compact()
        .with_writer(non_blocking)
        .with_filter(env_filter)
        .boxed();

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();
}

fn get_log_dir(app_name: String) -> PathBuf {
    let proj_dirs = ProjectDirs::from("", "", app_name.as_str()).unwrap();
    proj_dirs.data_dir().join("logs")
}
