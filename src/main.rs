use ocr_api::{build_state, router, Config};

#[tokio::main]
async fn main() {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());

    let cfg = Config {
        model_dir: env("OCR_MODEL_DIR").unwrap_or_else(|| "models/tiny".into()).into(),
        pool_size: env("OCR_POOL_SIZE").and_then(|v| v.parse().ok()).unwrap_or(2),
        api_key: env("OCR_API_KEY"),
        allowed_hosts: env("OCR_ALLOWED_HOSTS")
            .map(|v| v.split(',').map(|h| h.trim().to_owned()).filter(|h| !h.is_empty()).collect())
            .unwrap_or_default(),
    };
    let addr = env("OCR_LISTEN").unwrap_or_else(|| "127.0.0.1:8895".into());

    let state = build_state(cfg).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| {
        eprintln!("error: cannot listen on {addr}: {e}");
        std::process::exit(1);
    });
    eprintln!("ocr-api listening on http://{addr}  (POST /v1/ocr)");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await
        .unwrap();
}
