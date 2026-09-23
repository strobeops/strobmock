use std::pin::Pin;

use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

pub mod pb {
    tonic::include_proto!("mock");
}

pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("mock_descriptor");

use pb::echo_service_server::{EchoService, EchoServiceServer};
use pb::{EchoRequest, EchoResponse};

/// Type-erased response stream shared by the server-streaming and
/// bidirectional-streaming RPCs.
type EchoStream = Pin<Box<dyn Stream<Item = Result<EchoResponse, Status>> + Send + 'static>>;

/// gRPC benchmark service: mirrors payloads and streams on demand.
#[derive(Debug, Default, Clone, Copy)]
pub struct EchoServiceImpl;

#[tonic::async_trait]
impl EchoService for EchoServiceImpl {
    type ServerStreamEchoStream = EchoStream;
    type BidiEchoStream = EchoStream;

    async fn echo(&self, request: Request<EchoRequest>) -> Result<Response<EchoResponse>, Status> {
        let req = request.into_inner();
        tracing::debug!(message = %req.message, len = req.payload.len(), "gRPC Echo");
        Ok(Response::new(EchoResponse {
            message: req.message,
            payload: req.payload,
        }))
    }

    async fn server_stream_echo(
        &self,
        request: Request<EchoRequest>,
    ) -> Result<Response<Self::ServerStreamEchoStream>, Status> {
        let req = request.into_inner();
        let repeat = req.repeat.max(1);
        let responses: Vec<_> = (0..repeat)
            .map(|_| {
                Ok(EchoResponse {
                    message: req.message.clone(),
                    payload: req.payload.clone(),
                })
            })
            .collect();
        Ok(Response::new(Box::pin(tokio_stream::iter(responses))))
    }

    async fn bidi_echo(
        &self,
        request: Request<Streaming<EchoRequest>>,
    ) -> Result<Response<Self::BidiEchoStream>, Status> {
        let out = request.into_inner().map(|item| {
            item.map(|req| EchoResponse {
                message: req.message,
                payload: req.payload,
            })
        });
        Ok(Response::new(Box::pin(out)))
    }
}

/// Starts the gRPC EchoService (with server reflection) on an existing
/// listener, draining in-flight streams once `signal` resolves.
///
/// Reflection lets benchmark clients such as `grpcurl` introspect
/// `mock.EchoService` without carrying pre-compiled stubs.
pub async fn serve<F>(
    listener: tokio::net::TcpListener,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    tonic::transport::Server::builder()
        .add_service(EchoServiceServer::new(EchoServiceImpl))
        .add_service(reflection)
        .serve_with_incoming_shutdown(incoming, signal)
        .await?;

    Ok(())
}
