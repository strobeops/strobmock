use axum::{Router, body::Bytes, extract::Path, http::StatusCode, routing::get};

const MAX_SIZE: usize = 104_857_600; // 100 MB

pub fn app() -> Router {
    Router::new()
        .route("/echo", axum::routing::post(http_echo))
        .route("/bytes/{size}", get(http_bytes))
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
