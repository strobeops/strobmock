use std::net::SocketAddr;
use std::sync::Arc;

use bytes::{BufMut, Bytes, BytesMut};
use rustls::pki_types::CertificateDer;
use strobmock::http3::{self, Http3Error};

type TestResult<T> = Result<T, Http3Error>;

/// Spawns the HTTP/3 server on an ephemeral loopback UDP port with a
/// never-ending shutdown signal, returning the bound address and the
/// generated certificate so the client can trust it.
async fn spawn_server() -> TestResult<(
    SocketAddr,
    CertificateDer<'static>,
    tokio::task::JoinHandle<Result<(), Http3Error>>,
)> {
    let (cert, key) = http3::ephemeral_cert("127.0.0.1")?;
    let config = http3::server_config(cert.clone(), key)?;
    let endpoint = quinn::Endpoint::server(config, "127.0.0.1:0".parse()?)?;
    let addr = endpoint.local_addr()?;

    let handle = tokio::spawn(http3::serve(endpoint, std::future::pending::<()>()));
    Ok((addr, cert, handle))
}

struct Client {
    send_request: h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>,
    driver: tokio::task::JoinHandle<()>,
    endpoint: quinn::Endpoint,
}

impl Client {
    fn shutdown(self) {
        self.driver.abort();
        self.endpoint
            .close(quinn::VarInt::from_u32(0), b"test complete");
    }
}

async fn connect(addr: SocketAddr, cert: &CertificateDer<'static>) -> TestResult<Client> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.clone())?;

    let mut tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls)?;
    let mut client_config = quinn::ClientConfig::new(Arc::new(crypto));
    client_config.transport_config(Arc::new(http3::benchmark_transport()));

    let mut endpoint = quinn::Endpoint::client("127.0.0.1:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    let conn = endpoint.connect(addr, "localhost")?.await?;
    let (mut driver, send_request) = h3::client::new(h3_quinn::Connection::new(conn)).await?;
    let driver = tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    Ok(Client {
        send_request,
        driver,
        endpoint,
    })
}

async fn roundtrip(
    send_request: &mut h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>,
    port: u16,
    method: http::Method,
    path: &str,
    body: Option<Bytes>,
) -> TestResult<(http::StatusCode, Bytes)> {
    let uri = format!("https://localhost:{port}{path}");
    let req = http::Request::builder().method(method).uri(uri).body(())?;
    let mut stream = send_request.send_request(req).await?;

    if let Some(payload) = body {
        stream.send_data(payload).await?;
    }
    stream.finish().await?;

    let resp = stream.recv_response().await?;
    let mut out = BytesMut::new();
    while let Some(chunk) = stream.recv_data().await? {
        out.put(chunk);
    }

    Ok((resp.status(), out.freeze()))
}

#[tokio::test]
async fn test_h3_echo_roundtrip() -> TestResult<()> {
    let (addr, cert, server) = spawn_server().await?;
    let mut client = connect(addr, &cert).await?;

    let payload = Bytes::from_static(b"hello http3 echo");
    let (status, echoed) = roundtrip(
        &mut client.send_request,
        addr.port(),
        http::Method::POST,
        "/echo",
        Some(payload.clone()),
    )
    .await?;

    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(echoed, payload);

    client.shutdown();
    server.abort();
    Ok(())
}

#[tokio::test]
async fn test_h3_health_and_not_found() -> TestResult<()> {
    let (addr, cert, server) = spawn_server().await?;
    let mut client = connect(addr, &cert).await?;

    let (status, body) = roundtrip(
        &mut client.send_request,
        addr.port(),
        http::Method::GET,
        "/health",
        None,
    )
    .await?;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(body, Bytes::from_static(br#"{"status":"ok"}"#));

    let (status, _) = roundtrip(
        &mut client.send_request,
        addr.port(),
        http::Method::GET,
        "/missing",
        None,
    )
    .await?;
    assert_eq!(status, http::StatusCode::NOT_FOUND);

    client.shutdown();
    server.abort();
    Ok(())
}

#[tokio::test]
async fn test_h3_multiplexed_streams() -> TestResult<()> {
    let (addr, cert, server) = spawn_server().await?;
    let mut client = connect(addr, &cert).await?;

    // Open all streams up front on a single QUIC connection, then drive
    // them concurrently to exercise HTTP/3 stream multiplexing.
    let mut streams = Vec::with_capacity(8);
    for _ in 0..8 {
        let uri = format!("https://localhost:{}/echo", addr.port());
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri(uri)
            .body(())?;
        streams.push(client.send_request.send_request(req).await?);
    }

    let mut tasks = tokio::task::JoinSet::new();
    for (idx, mut stream) in streams.into_iter().enumerate() {
        let payload = Bytes::from(vec![idx as u8; 4096]);
        let expected = payload.clone();
        tasks.spawn(async move {
            stream.send_data(payload).await?;
            stream.finish().await?;

            let resp = stream.recv_response().await?;
            let mut out = BytesMut::new();
            while let Some(chunk) = stream.recv_data().await? {
                out.put(chunk);
            }
            Ok::<_, Http3Error>((resp.status(), out.freeze(), expected))
        });
    }

    let mut completed = 0;
    while let Some(joined) = tasks.join_next().await {
        let (status, echoed, expected) = joined??;
        assert_eq!(status, http::StatusCode::OK);
        assert_eq!(echoed, expected, "stream payloads must not cross-talk");
        completed += 1;
    }
    assert_eq!(completed, 8);

    client.shutdown();
    server.abort();
    Ok(())
}

#[tokio::test]
async fn test_h3_large_payload_echo() -> TestResult<()> {
    let (addr, cert, server) = spawn_server().await?;
    let mut client = connect(addr, &cert).await?;

    let payload = Bytes::from(vec![0x5A; 1024 * 1024]);
    let (status, echoed) = roundtrip(
        &mut client.send_request,
        addr.port(),
        http::Method::POST,
        "/echo",
        Some(payload.clone()),
    )
    .await?;

    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(echoed, payload);

    client.shutdown();
    server.abort();
    Ok(())
}
