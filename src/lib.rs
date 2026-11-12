//! Mistral-compatible `POST /v1/ocr` backed by rusto-rs (PP-OCRv6 Tiny).
//!
//! v1 is lines-only: `pages[].markdown` is the recognized lines, in the order
//! rusto returns them. No layout blocks, tables or images yet. Each page also
//! carries a non-Mistral `lines` array (text, score, box) that Mistral clients
//! ignore.

use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusto::{DetectTextResult, ImageSource, InitializeConfig, OcrRunOptions, RustO};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{Mutex, Semaphore};

pub const DEFAULT_MODEL: &str = "rusto-ppocrv6-tiny";
const MAX_DOC_BYTES: usize = 32 * 1024 * 1024;

pub struct Config {
    pub model_dir: PathBuf,
    pub pool_size: usize,
    pub api_key: Option<String>,
    /// Hosts `document_url` / `image_url` may be fetched from. Empty = no remote fetch.
    pub allowed_hosts: Vec<String>,
}

pub struct AppState {
    engines: Mutex<Vec<RustO>>,
    permits: Semaphore,
    api_key: Option<String>,
    allowed_hosts: Vec<String>,
}

pub fn build_state(cfg: Config) -> Result<Arc<AppState>, String> {
    let det = cfg.model_dir.join("det.onnx");
    let rec = cfg.model_dir.join("rec.onnx");
    let dict = cfg.model_dir.join("dict.txt");
    for p in [&det, &rec, &dict] {
        if !p.is_file() {
            return Err(format!("missing model file: {}", p.display()));
        }
    }
    let size = cfg.pool_size.clamp(1, 64);
    let mut engines = Vec::with_capacity(size);
    for _ in 0..size {
        let engine = RustO::initialize(InitializeConfig::ppv6(det.clone(), rec.clone(), dict.clone()))
            .map_err(|e| format!("rusto init failed: {e}"))?;
        engines.push(engine);
    }
    Ok(Arc::new(AppState {
        engines: Mutex::new(engines),
        permits: Semaphore::new(size),
        api_key: cfg.api_key,
        allowed_hosts: cfg.allowed_hosts,
    }))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/ocr", post(ocr))
        .layer(DefaultBodyLimit::max(MAX_DOC_BYTES * 2))
        .with_state(state)
}

// ---- request / response ----------------------------------------------------

#[derive(Deserialize)]
struct OcrRequest {
    model: Option<String>,
    document: Document,
}

#[derive(Deserialize)]
struct Document {
    #[serde(rename = "type")]
    kind: String,
    document_url: Option<UrlField>,
    image_url: Option<UrlField>,
}

/// Mistral accepts the URL either as a bare string or as `{"url": "..."}`.
#[derive(Deserialize)]
#[serde(untagged)]
enum UrlField {
    Plain(String),
    Object { url: String },
}

impl UrlField {
    fn url(&self) -> &str {
        match self {
            UrlField::Plain(u) | UrlField::Object { url: u } => u,
        }
    }
}

#[derive(Serialize)]
struct OcrResponse {
    pages: Vec<Page>,
    model: String,
    usage_info: UsageInfo,
}

#[derive(Serialize)]
struct Page {
    index: u32,
    markdown: String,
    images: Vec<serde_json::Value>,
    dimensions: Dimensions,
    /// Not part of the Mistral schema; ignored by Mistral clients.
    lines: Vec<Line>,
}

#[derive(Serialize)]
struct Dimensions {
    dpi: Option<u32>,
    height: u32,
    width: u32,
}

#[derive(Serialize)]
struct Line {
    text: String,
    score: f32,
    /// Four corners: top-left, top-right, bottom-right, bottom-left.
    #[serde(rename = "box")]
    corners: [(f32, f32); 4],
}

#[derive(Serialize)]
struct UsageInfo {
    pages_processed: u32,
    doc_size_bytes: usize,
}

// ---- errors (Mistral error envelope) ---------------------------------------

struct ApiError(StatusCode, &'static str, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({
            "object": "error",
            "message": self.2,
            "type": self.1,
            "param": null,
            "code": self.0.as_u16().to_string(),
        });
        (self.0, Json(body)).into_response()
    }
}

fn bad_request(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, "invalid_request_error", msg.into())
}

// ---- handler -----------------------------------------------------------------

async fn ocr(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<OcrRequest>,
) -> Result<Json<OcrResponse>, ApiError> {
    authorize(&state, &headers)?;

    let url = match req.document.kind.as_str() {
        "image_url" => req.document.image_url.as_ref(),
        "document_url" => req.document.document_url.as_ref(),
        other => return Err(bad_request(format!("unsupported document type: {other}"))),
    }
    .ok_or_else(|| bad_request("document is missing its url field"))?
    .url()
    .to_owned();

    let bytes = load_document(&state, &url).await?;
    if bytes.starts_with(b"%PDF") {
        return Err(bad_request("PDF input is not supported yet; send an image"));
    }
    let size = imagesize::blob_size(&bytes)
        .map_err(|_| bad_request("document is not a recognizable image"))?;

    let doc_size = bytes.len();
    let lines = run_ocr(&state, bytes).await?;
    let markdown = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");

    Ok(Json(OcrResponse {
        pages: vec![Page {
            index: 0,
            markdown,
            images: vec![],
            dimensions: Dimensions { dpi: None, height: size.height as u32, width: size.width as u32 },
            lines,
        }],
        model: req.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        usage_info: UsageInfo { pages_processed: 1, doc_size_bytes: doc_size },
    }))
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(key) = &state.api_key else { return Ok(()) };
    let ok = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|got| got == key);
    if ok {
        Ok(())
    } else {
        Err(ApiError(StatusCode::UNAUTHORIZED, "authentication_error", "invalid API key".into()))
    }
}

async fn load_document(state: &AppState, url: &str) -> Result<Vec<u8>, ApiError> {
    if let Some(rest) = url.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',').ok_or_else(|| bad_request("malformed data URI"))?;
        if !meta.ends_with(";base64") {
            return Err(bad_request("data URI must be base64-encoded"));
        }
        let bytes = STANDARD
            .decode(data.trim())
            .map_err(|_| bad_request("invalid base64 in data URI"))?;
        return check_size(bytes);
    }

    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(bad_request("document url must be a data: URI or http(s) URL"));
    }
    let host = url
        .split("://")
        .nth(1)
        .and_then(|r| r.split(['/', '?', '#']).next())
        .and_then(|authority| authority.rsplit('@').next())
        .map(|h| h.split(':').next().unwrap_or(h).to_ascii_lowercase())
        .unwrap_or_default();
    if !state.allowed_hosts.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return Err(bad_request(format!(
            "fetching from host '{host}' is not allowed; send a data: URI or add it to OCR_ALLOWED_HOSTS"
        )));
    }

    let url = url.to_owned();
    let fetched = tokio::task::spawn_blocking(move || {
        use std::io::Read;
        // No redirects: a redirect could leave the allowlisted host.
        let agent = ureq::AgentBuilder::new().redirects(0).timeout(std::time::Duration::from_secs(30)).build();
        let resp = agent.get(&url).call().map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        resp.into_reader().take(MAX_DOC_BYTES as u64 + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        Ok::<_, String>(buf)
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "server_error", e.to_string()))?
    .map_err(|e| bad_request(format!("could not fetch document: {e}")))?;
    check_size(fetched)
}

fn check_size(bytes: Vec<u8>) -> Result<Vec<u8>, ApiError> {
    if bytes.is_empty() {
        return Err(bad_request("document is empty"));
    }
    if bytes.len() > MAX_DOC_BYTES {
        return Err(bad_request("document exceeds the 32 MB limit"));
    }
    Ok(bytes)
}

async fn run_ocr(state: &Arc<AppState>, bytes: Vec<u8>) -> Result<Vec<Line>, ApiError> {
    let server_error = |m: String| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "server_error", m);

    let _permit = state.permits.acquire().await.map_err(|e| server_error(e.to_string()))?;
    let mut engine = state.engines.lock().await.pop().ok_or_else(|| server_error("engine pool empty".into()))?;

    // Inference is CPU-bound and cannot be interrupted; run it off the async threads
    // and always return the engine to the pool.
    let (engine, result) = tokio::task::spawn_blocking(move || {
        let r = engine.detect_text(&ImageSource::Bytes(bytes), &OcrRunOptions::default());
        (engine, r)
    })
    .await
    .map_err(|e| server_error(e.to_string()))?;
    state.engines.lock().await.push(engine);

    match result {
        Ok(DetectTextResult::Structured(rs)) => Ok(rs
            .into_iter()
            .map(|r| Line { text: r.text, score: r.score, corners: r.box_points })
            .collect()),
        Ok(DetectTextResult::Spatial(_)) => Err(server_error("unexpected spatial output".into())),
        Err(e) => Err(bad_request(format!("OCR failed: {e}"))),
    }
}
