# strobmock

HTTP echo server and dynamic byte generator for benchmarking [strobengine](https://github.com/strobeops/strobengine).

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/echo` | Mirrors request body back (zero-copy) |
| GET | `/bytes/{size}` | Returns zero-filled buffer of `size` bytes (max 100 MB) |
| GET | `/sse` | Streams "ping" events every 100ms (SSE) |

## Project Structure

```
src/
  main.rs      # CLI entry point and server startup
  lib.rs       # Router, handlers, and core logic
tests/
  http_tests.rs  # Integration tests (echo, bytes, error handling)
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
  -p, --port <PORT>      Port to listen on [default: 8080]
  -b, --host <HOST>      Host address to bind to [default: 0.0.0.0]
  -h, --help             Print help
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
```

## Testing

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Integration tests in `tests/http_tests.rs` cover:

- `POST /echo` — body echoed back with 200 OK
- `GET /bytes/1024` — returns 1024 bytes with `application/octet-stream`
- `GET /bytes/104857601` — returns 400 Bad Request (exceeds 100 MB limit)

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Commit with conventional commits (`git commit -m "feat: add my feature"`)
4. Push to the branch (`git push origin feat/my-feature`)
5. Open a Pull Request

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
