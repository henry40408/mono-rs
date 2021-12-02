use std::net::SocketAddr;
use warp::Filter;

use askama::Template;

#[derive(Template)]
#[template(path = "index.html.j2")]
struct IndexTemplate {}

/// Start web server
pub async fn start_server(bind: &str) -> anyhow::Result<()> {
    let bind: SocketAddr = bind.parse()?;

    let root = warp::path::end()
        .map(|| {
            let t = IndexTemplate {};
            warp::reply::html(format!("{}", t.render().unwrap()))
        })
        .with(warp::log("bk"));

    warp::serve(root).run(bind).await;

    Ok(())
}
