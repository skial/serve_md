extern crate alloc;

use alloc::sync::Arc;
use std::net::SocketAddr;
use axum::{ Router, routing::get, };
use serve_md::state::State as Cli;
use clap::Parser as CliParser;
use tokio::signal;
use serve_md::determine;

#[tokio::main]
async fn main() {
    let mut cli = Cli::parse();
    cli.load_config();
    cli.set_missing();

    #[cfg(debug_assertions)]
    dbg!(&cli);

    let state = Arc::new(cli);
    
    // As far as I can tell, axum can't match paths with
    // file extensions? `:file.html` or `:file.md`.
    let routes = Router::new()
        .route("/:path", get({
            let shared_state = Arc::clone(&state);
            move |path| determine(path, shared_state)
        }))
    ;

    let addr = SocketAddr::from(([127, 0, 0, 1], state.port));
    println!("starting server on 127.0.0.1:{}", state.port);
    axum::Server::bind(&addr)
        .serve(routes.into_make_service())
        // @see https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

/// Graceful shutdown from <https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs>
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = core::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
}