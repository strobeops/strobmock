use axum::{Router, body::Body};
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn app() -> Router {
    strobmock::app()
}

#[tokio::test]
async fn test_echo_endpoint() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/echo")
                .method("POST")
                .body(Body::from("hello world"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(body.as_ref(), b"hello world");
}

#[tokio::test]
async fn test_bytes_endpoint_valid() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/bytes/1024")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "application/octet-stream"
    );

    let body = BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(body.len(), 1024);
}

#[tokio::test]
async fn test_bytes_endpoint_exceeds_limit() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/bytes/104857601")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
