# strobmock

Multi-protocol HTTP and gRPC mock server for benchmarking [strobengine](https://github.com/strobeops/strobengine).

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

## Project Structure

```
src/
  main.rs      # CLI entry point and server startup
  lib.rs       # HTTP router, handlers, and core logic
  grpc.rs      # gRPC EchoService (unary, server-stream, bidi) + reflection
  shutdown.rs  # Shared ctrl-c drain signal for HTTP and gRPC
proto/
  mock.proto   # EchoService contract
tests/
  http_tests.rs   # HTTP integration tests
  grpc_tests.rs   # gRPC integration tests (ephemeral loopback ports)
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
```

## Testing

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Integration tests in `tests/http_tests.rs` and `tests/grpc_tests.rs` run entirely against an in-process / ephemeral loopback server (no external URLs) and cover:

- `POST /echo` — body echoed back with 200 OK
- `GET /bytes/1024` — returns 1024 bytes with `application/octet-stream`
- `GET /bytes/104857601` — returns 400 Bad Request (exceeds 100 MB limit)
- `GET /health`, `GET /sse`, `GET /ws` — route wiring and response contracts
- gRPC `Echo` (unary), `ServerStreamEcho`, and `BidiEcho` round-trips

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Commit with conventional commits (`git commit -m "feat: add my feature"`)
4. Push to the branch (`git push origin feat/my-feature`)
5. Open a Pull Request

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
