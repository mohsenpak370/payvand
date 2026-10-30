//! Vercel Rust runtime entrypoint: mounts the shared router behind
//! `VercelLayer` so the whole API is served by one serverless function.
//!
//! `@vercel/rust` zero-config deploys `src/main.rs`; the local dev server
//! lives in `src/bin/server.rs` (`pnpm api` → `cargo run --bin server`).

use payvand_server::app;
use tower::ServiceBuilder;
use vercel_runtime::axum::VercelLayer;
use vercel_runtime::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    eprintln!("[axum] entrypoint starting");
    let router = app().await;
    eprintln!("[axum] router built (db open + migrations done)");
    let service = ServiceBuilder::new()
        .layer(VercelLayer::new())
        .service(router);

    eprintln!("[axum] handing off to vercel_runtime");
    vercel_runtime::run(service).await
}
