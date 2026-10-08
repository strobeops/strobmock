use std::net::IpAddr;
use std::sync::Arc;

use bytes::{Buf, Bytes};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Boxed error type shared by the HTTP/3 target (mirrors `grpc::serve`).
pub type Http3Error = Box<dyn std::error::Error + Send + Sync>;

/// Subject alternative names for the ephemeral certificate: loopback names
/// are always included; `host` contributes only when it is a concrete IP.
fn certificate_sans(host: &str) -> Vec<String> {
    let mut sans = vec![
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        "::1".to_owned(),
    ];
    if let Ok(ip) = host.parse::<IpAddr>()
        && !ip.is_unspecified()
    {
        let canonical = ip.to_string();
        if !sans.contains(&canonical) {
            sans.push(canonical);
        }
    }
    sans
}

/// Generates an ephemeral self-signed certificate on startup, valid for
/// loopback clients (`localhost`, `127.0.0.1`, `::1`) plus `host` when it
/// is a concrete IP literal.
pub fn ephemeral_cert(
    host: &str,
) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), Http3Error> {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(certificate_sans(host))?;
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    Ok((cert.der().clone(), key))
}

/// QUIC transport limits sized for benchmark traffic: large stream and
/// connection receive windows avoid flow-control stalls while echoing, and
/// a 30 s idle timeout keeps connections alive between bursts.
pub fn benchmark_transport() -> quinn::TransportConfig {
    let mut transport = quinn::TransportConfig::default();
    transport
        .max_idle_timeout(Some(quinn::IdleTimeout::from(quinn::VarInt::from_u32(
            30_000,
        ))))
        .stream_receive_window(quinn::VarInt::from_u32(8 * 1024 * 1024))
        .receive_window(quinn::VarInt::from_u32(32 * 1024 * 1024))
        .max_concurrent_bidi_streams(quinn::VarInt::from_u32(1024));
    transport
}

/// Builds a QUIC server configuration with ALPN `h3` and the benchmark
/// transport limits.
pub fn server_config(
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
) -> Result<quinn::ServerConfig, Http3Error> {
    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls)?;
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    config.transport = Arc::new(benchmark_transport());
    Ok(config)
}

/// Runs the HTTP/3 echo server on a bound QUIC endpoint, draining in-flight
/// streams once `signal` resolves.
pub async fn serve<F>(endpoint: quinn::Endpoint, signal: F) -> Result<(), Http3Error>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let mut signal = std::pin::pin!(signal);
    loop {
        tokio::select! {
            _ = signal.as_mut() => break,
            incoming = endpoint.accept() => match incoming {
                Some(connecting) => {
                    tokio::spawn(handle_connection(connecting));
                }
                None => break,
            },
        }
    }
    endpoint.close(quinn::VarInt::from_u32(0), b"strobmock shutting down");
    endpoint.wait_idle().await;
    Ok(())
}

async fn handle_connection(connecting: quinn::Incoming) {
    let conn = match connecting.await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::debug!(error = %err, "QUIC handshake failed");
            return;
        }
    };

    let mut h3_conn = match h3::server::Connection::new(h3_quinn::Connection::new(conn)).await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::debug!(error = %err, "HTTP/3 handshake failed");
            return;
        }
    };

    loop {
        match h3_conn.accept().await {
            Ok(Some(resolver)) => {
                tokio::spawn(async move {
                    if let Err(err) = handle_request(resolver).await {
                        tracing::debug!(error = %err, "HTTP/3 request failed");
                    }
                });
            }
            Ok(None) => break,
            Err(err) => {
                tracing::debug!(error = %err, "HTTP/3 accept failed");
                break;
            }
        }
    }
}

async fn handle_request<C>(
    resolver: h3::server::RequestResolver<C, Bytes>,
) -> Result<(), Http3Error>
where
    C: h3::quic::Connection<Bytes>,
{
    let (req, mut stream) = resolver.resolve_request().await?;

    match req.uri().path() {
        // Response headers go out first, then request-body chunks are
        // reflected to the response stream as they arrive (full duplex,
        // no buffering). H3 delimits the response body by FIN, so no
        // content-length is needed.
        "/echo" => {
            let mut builder = http::Response::builder().status(http::StatusCode::OK);
            if let Some(content_type) = req.headers().get(http::header::CONTENT_TYPE) {
                builder = builder.header(http::header::CONTENT_TYPE, content_type.clone());
            }
            stream.send_response(builder.body(())?).await?;

            while let Some(mut chunk) = stream.recv_data().await? {
                if !chunk.has_remaining() {
                    continue;
                }
                let payload = chunk.copy_to_bytes(chunk.remaining());
                stream.send_data(payload).await?;
            }
            // drain trailing frames so nothing is left unread on the stream
            let _ = stream.recv_trailers().await;
            stream.finish().await?;
        }
        "/health" => {
            drain_body(&mut stream).await?;
            let resp = http::Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(())?;
            stream.send_response(resp).await?;
            stream
                .send_data(Bytes::from_static(br#"{"status":"ok"}"#))
                .await?;
            stream.finish().await?;
        }
        _ => {
            drain_body(&mut stream).await?;
            let resp = http::Response::builder()
                .status(http::StatusCode::NOT_FOUND)
                .body(())?;
            stream.send_response(resp).await?;
            stream.finish().await?;
        }
    }

    Ok(())
}

async fn drain_body<S>(stream: &mut h3::server::RequestStream<S, Bytes>) -> Result<(), Http3Error>
where
    S: h3::quic::RecvStream,
{
    while stream.recv_data().await?.is_some() {}
    let _ = stream.recv_trailers().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sans_always_include_loopback() {
        let sans = certificate_sans("0.0.0.0");
        assert_eq!(sans, ["localhost", "127.0.0.1", "::1"]);
    }

    #[test]
    fn sans_add_concrete_host_ip_once() {
        let sans = certificate_sans("192.168.1.7");
        assert!(sans.contains(&"192.168.1.7".to_owned()));

        let loopback = certificate_sans("127.0.0.1");
        assert_eq!(
            loopback.iter().filter(|s| *s == "127.0.0.1").count(),
            1,
            "already-known IP must not be duplicated"
        );
    }

    #[test]
    fn sans_keep_canonical_ipv6_form() {
        let sans = certificate_sans("::1");
        assert_eq!(
            sans.iter().filter(|s| *s == "::1").count(),
            1,
            "IPv6 loopback must stay canonical and unbracketed"
        );
    }

    #[test]
    fn sans_ignore_unspecified_and_non_ip_hosts() {
        let wildcards = certificate_sans("::");
        assert_eq!(wildcards, ["localhost", "127.0.0.1", "::1"]);

        let dns = certificate_sans("bench.example");
        assert!(!dns.contains(&"bench.example".to_owned()));
    }

    #[test]
    fn ephemeral_cert_generates_der_material() {
        let (cert, key) = ephemeral_cert("127.0.0.1").expect("cert generation");
        assert!(!cert.as_ref().is_empty());
        assert!(!key.secret_der().is_empty());
    }

    #[test]
    fn server_config_accepts_generated_material() {
        let (cert, key) = ephemeral_cert("localhost").expect("cert generation");
        assert!(server_config(cert, key).is_ok());
    }
}
