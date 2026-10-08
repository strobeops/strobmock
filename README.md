# strobmock

Multi-protocol HTTP, gRPC, and HTTP/3 (QUIC) mock server for benchmarking [strobengine](https://github.com/strobeops/strobengine).

## Endpoints

### HTTP

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Baseline control target (`{"status": "ok"}`) |
| POST | `/echo` | Mirrors request body back (zero-copy) |
| GET | `/bytes/{size}` | Returns zero-filled buffer of `size` bytes (max 100 MB) |
| GET | `/sse` | Streams "ping" events every 100ms (SSE) |
| GET (Upgrade) | `/ws` | WebSocket echo target (Text, Binary, Ping/Pong) |

### gRPC (`mock.EchoService`, enabled with `--grpc`)

| RPC | Type | Description |
|-----|------|-------------|
| `Echo` | Unary | Mirrors `message` and `payload` back |
| `ServerStreamEcho` | Server streaming | Streams `repeat` echoed responses |
| `BidiEcho` | Bidirectional streaming | Mirrors each streamed request |

Server reflection is enabled, so `grpcurl` and other dynamic clients work without pre-compiled stubs.

### HTTP/3 (enabled with `--h3`, UDP `:8443`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/echo` | Streaming pass-through echo: each request-body chunk is reflected as it arrives |
| GET | `/health` | Baseline control target (`{"status": "ok"}`) |

Any other path returns 404. The TLS certificate is generated ephemerally with `rcgen` on every startup (self-signed; SANs cover `localhost`, `127.0.0.1`, `::1`, plus `--host` when it is a concrete IP), so clients must skip verification (`curl -k`) or trust the printed certificate.

## Project Structure

```
src/
  main.rs      # CLI entry point and server startup
  lib.rs       # HTTP router, handlers, and core logic
  grpc.rs      # gRPC EchoService (unary, server-stream, bidi) + reflection
  http3.rs     # HTTP/3 (QUIC) echo target: ephemeral TLS, h3 routes
  shutdown.rs  # Shared ctrl-c drain signal for all servers
proto/
  mock.proto   # EchoService contract
tests/
  http_tests.rs   # HTTP integration tests
  grpc_tests.rs   # gRPC integration tests (ephemeral loopback ports)
  http3_tests.rs  # HTTP/3 integration tests (ephemeral UDP ports)
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.53.1 | Async runtime |
| axum | 0.8.9 | HTTP server framework |
| bytes | 1.12.1 | Zero-copy byte buffer |
| clap | 4.6.6 | CLI argument parsing |
| tracing | 0.1.44 | Structured logging |
| tracing-subscriber | 0.3.23 | Log formatting |
| tonic | 0.14.6 | gRPC over HTTP/2 |
| tonic-prost | 0.14.6 | Prost codec for tonic |
| prost | 0.14.4 | Protocol Buffers implementation |
| tonic-reflection | 0.14.6 | gRPC server reflection |
| quinn | 0.11 | QUIC transport |
| h3 | 0.0.8 | HTTP/3 protocol implementation |
| h3-quinn | 0.0.10 | Quinn adapter for h3 |
| rcgen | 0.14 | Ephemeral self-signed TLS certificates |
| rustls | 0.23 | TLS (ring-only, aligned with quinn) |
| http | 1.5 | HTTP types shared by HTTP/3 handlers |

`tonic-prost-build` and `protobuf-src` are build-dependencies; `protobuf-src` vendors `protoc` so no system protobuf toolchain is required.

## Installation

### From source

```bash
git clone https://github.com/strobeops/strobmock.git
cd strobmock
cargo build --release
```

The binary will be at `target/release/strobmock`.

### Via cargo install (if published)

```bash
cargo install strobmock
```

## Usage

```bash
strobmock [OPTIONS]

Options:
  -p, --port <PORT>          Port to listen on [default: 8080]
  -b, --host <HOST>          Host address to bind to [default: 0.0.0.0]
  -v, --verbose              Increase logging verbosity (-v for DEBUG, -vv for TRACE)
  -q, --quiet                Quiet mode (only WARN and ERROR logs)
      --log-file <LOG_FILE>  Optional file path to append logs to
      --grpc                 Enable the gRPC EchoService alongside HTTP
      --grpc-port <PORT>     gRPC listen port [default: 50051]
      --h3                   Enable the HTTP/3 (QUIC) target alongside HTTP
      --h3-port <H3_PORT>    HTTP/3 listen port over UDP [default: 8443]
  -h, --help                 Print help
```

### Examples

```bash
# Start on default port
strobmock

# Custom host and port
strobmock --host 127.0.0.1 --port 3000

# Echo a payload
curl -X POST http://localhost:8080/echo -d "hello world"

# Generate 1MB of zero-filled bytes
curl http://localhost:8080/bytes/1048576

# Start HTTP + gRPC together, then introspect the service with grpcurl
strobmock --grpc
grpcurl -plaintext localhost:50051 list
grpcurl -plaintext -d '{"message":"hi","payload":"AAEC"}' localhost:50051 mock.EchoService/Echo

# Start HTTP + HTTP/3, then echo over QUIC (curl needs an HTTP/3 build;
# -k skips verification of the ephemeral self-signed certificate)
strobmock --h3
curl --http3-only -k -X POST https://localhost:8443/echo -d "hello quic"
curl --http3-only -k https://localhost:8443/health
```

## Testing

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Integration tests in `tests/http_tests.rs`, `tests/grpc_tests.rs`, and `tests/http3_tests.rs` run entirely against an in-process / ephemeral loopback server (no external URLs) and cover:

- `POST /echo` — body echoed back with 200 OK
- `GET /bytes/1024` — returns 1024 bytes with `application/octet-stream`
- `GET /bytes/104857601` — returns 400 Bad Request (exceeds 100 MB limit)
- `GET /health`, `GET /sse`, `GET /ws` — route wiring and response contracts
- gRPC `Echo` (unary), `ServerStreamEcho`, and `BidiEcho` round-trips
- HTTP/3 `/echo` round-trips (small, 1 MiB) and `/health` / 404 contracts
- HTTP/3 multiplexing: 8 concurrent streams on one QUIC connection

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Commit with conventional commits (`git commit -m "feat: add my feature"`)
4. Push to the branch (`git push origin feat/my-feature`)
5. Open a Pull Request

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
