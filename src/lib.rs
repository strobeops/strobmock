use axum::{
    Json, Router,
    body::Bytes,
    extract::Path,
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{any, get},
};
use serde_json::json;
use std::{convert::Infallible, time::Duration};
use tokio_stream::StreamExt;
use tower_http::trace::TraceLayer;

const MAX_SIZE: usize = 104_857_600; // 100 MB

pub fn app() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/echo", any(http_echo))
        .route("/bytes/{size}", get(http_bytes))
        .route("/sse", get(sse_handler))
        .layer(TraceLayer::new_for_http())
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn http_echo(body: Bytes) -> Bytes {
    body
}

async fn http_bytes(Path(size): Path<usize>) -> Result<Bytes, (StatusCode, &'static str)> {
    if size > MAX_SIZE {
        return Err((
            StatusCode::BAD_REQUEST,
            "Requested size exceeds 100MB limit",
        ));
    }
    Ok(Bytes::from(vec![0u8; size]))
}

async fn sse_handler() -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(
        Duration::from_millis(100),
    ))
    .map(|_| Ok(Event::default().event("message").data("ping")));

    Sse::new(stream).keep_alive(KeepAlive::default())
}
