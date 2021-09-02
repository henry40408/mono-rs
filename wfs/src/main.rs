use env_logger::Env;
use log::info;
use tokio::signal::unix::{signal, SignalKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("wait for SIGINT or SIGTERM");

    let mut int = signal(SignalKind::interrupt())?;
    let mut term = signal(SignalKind::terminate())?;

    tokio::select! {
        _ = int.recv() => info!("SIGINT received"),
        _ = term.recv() => info!("SIGTERM received"),
    }

    Ok(())
}
