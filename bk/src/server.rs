use std::net::SocketAddr;
use warp::Filter;

/// Start web server
pub async fn start_server(bind: &str) -> anyhow::Result<()> {
    let bind: SocketAddr = bind.parse()?;

    let root = warp::path::end()
        .map(|| format!("Ok"))
        .with(warp::log("bk"));

    warp::serve(root).run(bind).await;

    Ok(())
}
