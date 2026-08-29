use axum::{Router, body::Bytes, extract::Path, http::StatusCode, routing::get};
use clap::Parser;

const MAX_SIZE: usize = 104_857_600; // 100 MB

#[derive(Parser)]
#[command(
    name = "strobmock",
    about = "HTTP echo & byte generator for benchmarking"
)]
struct Args {
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    #[arg(short = 'b', long, default_value_t = String::from("0.0.0.0"))]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let addr: std::net::SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let app = Router::new()
        .route("/echo", axum::routing::post(http_echo))
        .route("/bytes/{size}", get(http_bytes));

    tracing::info!("Listening on http://{addr}");
    eprintln!("strobmock server running on http://{addr}");

    axum::serve(listener, app).await?;

    Ok(())
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
