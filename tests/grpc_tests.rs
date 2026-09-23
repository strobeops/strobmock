use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tonic::transport::Channel;

use strobmock::grpc::pb::echo_service_client::EchoServiceClient;
use strobmock::grpc::pb::{EchoRequest, EchoResponse};

/// Binds an ephemeral loopback port, spawns the gRPC server with a never-ending
/// shutdown signal, and returns the address plus the server task handle.
async fn spawn_server() -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let _ = strobmock::grpc::serve(listener, std::future::pending::<()>()).await;
    });
    (addr, handle)
}

async fn connect(addr: SocketAddr) -> EchoServiceClient<Channel> {
    EchoServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap()
}

async fn collect(mut stream: tonic::codec::Streaming<EchoResponse>) -> Vec<EchoResponse> {
    let mut out = Vec::new();
    while let Some(msg) = stream.message().await.unwrap() {
        out.push(msg);
    }
    out
}

#[tokio::test]
async fn test_grpc_unary_echo() {
    let (addr, handle) = spawn_server().await;
    let mut client = connect(addr).await;

    let response = client
        .echo(EchoRequest {
            message: "hello".into(),
            payload: vec![1, 2, 3],
            repeat: 0,
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(response.message, "hello");
    assert_eq!(response.payload, vec![1, 2, 3]);
    handle.abort();
}

#[tokio::test]
async fn test_grpc_server_stream() {
    let (addr, handle) = spawn_server().await;
    let mut client = connect(addr).await;

    let stream = client
        .server_stream_echo(EchoRequest {
            message: "tick".into(),
            payload: vec![9; 4],
            repeat: 3,
        })
        .await
        .unwrap()
        .into_inner();

    let responses = collect(stream).await;
    assert_eq!(responses.len(), 3);
    assert!(
        responses
            .iter()
            .all(|r| r.message == "tick" && r.payload == vec![9; 4])
    );
    handle.abort();
}

#[tokio::test]
async fn test_grpc_bidi_echo() {
    let (addr, handle) = spawn_server().await;
    let mut client = connect(addr).await;

    let requests = (0..5u32).map(|i| EchoRequest {
        message: format!("m{i}"),
        payload: vec![i as u8],
        repeat: 0,
    });

    let stream = client
        .bidi_echo(tokio_stream::iter(requests.collect::<Vec<_>>()))
        .await
        .unwrap()
        .into_inner();

    let responses = collect(stream).await;
    assert_eq!(responses.len(), 5);
    for (i, resp) in responses.iter().enumerate() {
        assert_eq!(resp.message, format!("m{i}"));
        assert_eq!(resp.payload, vec![i as u8]);
    }
    handle.abort();
}
