use env_logger::Env;

#[cfg(target_os = "windows")]
use log::error;

#[cfg(not(target_os = "windows"))]
use log::info;

#[cfg(not(target_os = "windows"))]
use tokio::signal::unix::{signal, SignalKind};

#[cfg(target_os = "windows")]
fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    error!("windows is NOT supported");
}

#[cfg(not(target_os = "windows"))]
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
