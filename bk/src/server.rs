use std::net::SocketAddr;
use warp::{Filter, Rejection, Reply};

use askama::Template;
use bk::entities::Scrape;

#[derive(Template)]
#[template(path = "index.html.j2")]
struct IndexTemplate {}

fn index() -> impl Reply {
    let t = IndexTemplate {};
    warp::reply::html(format!("{}", t.render().unwrap()))
}

fn index_filter() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path::end().map(|| index())
}

/// Start web server
pub async fn start_server(bind: &str) -> anyhow::Result<()> {
    let bind: SocketAddr = bind.parse()?;
    let root = index_filter();
    let app = root.with(warp::log("bk"));
    warp::serve(app).run(bind).await;
    Ok(())
}
