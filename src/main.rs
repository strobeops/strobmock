use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(
    name = "strobmock",
    about = "HTTP echo & byte generator for benchmarking"
)]
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let _guard = init_tracing(&args);

    let addr: std::net::SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let app = strobmock::app();

    tracing::info!("Listening on http://{addr}");
    eprintln!("strobmock server running on http://{addr}");

    axum::serve(listener, app).await?;

    Ok(())
}
