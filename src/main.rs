use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "strobmock", about = "HTTP & gRPC mock server for benchmarking")]
pub struct Args {
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    #[arg(short = 'b', long, default_value_t = String::from("0.0.0.0"))]
    pub host: String,

    /// Increase logging verbosity (-v for DEBUG, -vv for TRACE)
    #[arg(short = 'v', long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Quiet mode (only display WARN and ERROR logs)
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Optional file path to append logs to (e.g. ./strobmock.log)
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// Enable the gRPC EchoService benchmark target alongside HTTP
    #[arg(long)]
    pub grpc: bool,

    /// gRPC listen port (used only when --grpc is set)
    #[arg(long, default_value_t = 50051)]
    pub grpc_port: u16,
}

fn init_tracing(args: &Args) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter_level = if args.quiet {
        tracing::Level::WARN
    } else {
        match args.verbose {
            0 => tracing::Level::INFO,
            1 => tracing::Level::DEBUG,
            _ => tracing::Level::TRACE,
        }
    };

    let env_filter = EnvFilter::from_default_env().add_directive(filter_level.into());
    let stdout_layer = fmt::layer().with_filter(env_filter.clone());

    if let Some(ref log_path) = args.log_file {
        let file_name = log_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("strobmock.log"));
        let parent_dir = log_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));

        let file_appender = tracing_appender::rolling::never(parent_dir, file_name);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_filter(env_filter);

        tracing_subscriber::registry()
            .with(stdout_layer)
            .with(file_layer)
            .init();

        Some(guard)
    } else {
        tracing_subscriber::registry().with(stdout_layer).init();
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    let _guard = init_tracing(&args);

    let addr: std::net::SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;

    let (http_rx, grpc_rx) = strobmock::shutdown::signal_pair();
    let http_fut = axum::serve(listener, strobmock::app())
        .with_graceful_shutdown(strobmock::shutdown::wait(http_rx));

    eprintln!("strobmock server running on http://{local}");

    if args.grpc {
        let grpc_addr: std::net::SocketAddr =
            format!("{}:{}", args.host, args.grpc_port).parse()?;
        let grpc_listener = tokio::net::TcpListener::bind(grpc_addr).await?;
        let grpc_local = grpc_listener.local_addr()?;

        tracing::info!("gRPC EchoService listening on {grpc_local}");
        eprintln!("strobmock gRPC server running on {grpc_local}");

        let grpc_fut = strobmock::grpc::serve(grpc_listener, strobmock::shutdown::wait(grpc_rx));

        tokio::select! {
            r = http_fut => r?,
            r = grpc_fut => r?,
        }
    } else {
        http_fut.await?;
    }

    Ok(())
}
