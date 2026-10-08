use tokio::sync::watch;

/// Installs a single `ctrl_c` owner and returns the base receiver.
///
/// A background task flips the shared channel on the first `SIGINT`, letting
/// every server drain its in-flight connections without registering
/// competing signal handlers.
fn install_signal() -> watch::Receiver<bool> {
    let (tx, rx) = watch::channel(false);
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::info!("Shutdown signal received — draining connections");
            let _ = tx.send(true);
        }
    });
    rx
}

/// Returns one receiver per server, all driven by the same `ctrl_c` owner.
pub fn signals(count: usize) -> Vec<watch::Receiver<bool>> {
    let base = install_signal();
    (0..count).map(|_| base.clone()).collect()
}

/// Two receivers for the classic HTTP + gRPC pairing.
pub fn signal_pair() -> (watch::Receiver<bool>, watch::Receiver<bool>) {
    let base = install_signal();
    (base.clone(), base)
}

/// Resolves once the shutdown flag is set, or immediately if the sender drops.
pub async fn wait(mut rx: watch::Receiver<bool>) {
    while !*rx.borrow_and_update() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn signals_returns_requested_count() {
        assert_eq!(signals(3).len(), 3);
    }

    #[tokio::test]
    async fn signal_pair_starts_unset() {
        let (first, second) = signal_pair();
        assert!(!*first.borrow());
        assert!(!*second.borrow());
    }
}
