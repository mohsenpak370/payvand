use payvand_server::app;

/// Loads the shared `apps/web/.env` — the same file Nuxt reads — so
/// `SOLANA_CLUSTER` and friends configure both halves from one place. A
/// missing file is fine and variables already set in the shell are never
/// overwritten.
fn load_repo_env() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../web/.env");
    let _ = dotenvy::from_path(path);
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    load_repo_env();

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind BIND_ADDR");

    tracing::info!("payvand api listening on http://{addr}");
    let router = app().await;
    axum::serve(listener, router).await.expect("server exited");
}
