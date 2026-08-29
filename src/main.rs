use axum::Router;
use clap::Parser;

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
    let app = Router::new();

    tracing::info!("Listening on http://{addr}");

    axum::serve(listener, app).await?;

    Ok(())
}
