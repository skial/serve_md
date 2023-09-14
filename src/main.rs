#![allow(clippy::pedantic, clippy::correctness, clippy::perf, clippy::style, clippy::restriction)]

use std::{
    sync::Arc,
    net::SocketAddr,
};

use axum::{ 
    Router,
    routing::get,
};

use serve_md::state::State as Cli;
use clap::Parser as CliParser;

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
    axum::Server::bind(&addr)
        .serve(routes.into_make_service())
        .await
        .unwrap();
}