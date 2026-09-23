use tokio::sync::watch;

/// Installs a single `ctrl_c` owner and returns one receiver per server.
///
/// A background task flips the shared channel on the first `SIGINT`, allowing
/// both the HTTP and gRPC servers to drain their in-flight connections
/// independently without registering competing signal handlers.
pub fn signal_pair() -> (watch::Receiver<bool>, watch::Receiver<bool>) {
    let (tx, rx) = watch::channel(false);
    let http_rx = rx.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::info!("Shutdown signal received — draining connections");
            let _ = tx.send(true);
        }
    });
    (http_rx, rx)
}

/// Resolves once the shutdown flag is set, or immediately if the sender drops.
pub async fn wait(mut rx: watch::Receiver<bool>) {
    while !*rx.borrow_and_update() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}
