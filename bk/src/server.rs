use std::collections::HashMap;
use std::convert::Infallible;
use std::default::Default;
use std::net::SocketAddr;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

use askama::Template;
use bk::entities::{Scrape, SearchScrape};
use bk::{build_connection_pool, DBConnectionPool};

struct State {
    pool: DBConnectionPool,
}

#[derive(Template)]
#[template(path = "index.html.j2")]
struct IndexTemplate<'a> {
    scrapes: Vec<Scrape>,
    search: SearchScrape<'a>,
}

fn with_state(
    state: Arc<State>,
) -> impl Filter<Extract = (Arc<State>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn do_index(state: Arc<State>) -> anyhow::Result<String> {
    let conn = state.pool.get()?;
    let mut search = SearchScrape {
        users: Some(HashMap::new()),
        ..Default::default()
    };
    let scrapes = Scrape::search(&conn, &mut search)?;
    let t = IndexTemplate { scrapes, search };
    Ok(format!("{}", t.render()?))
}

fn index(state: Arc<State>) -> impl Reply {
    let body = match do_index(state) {
        Ok(r) => r,
        Err(e) => format!("{:?}", e),
    };
    warp::reply::html(body)
}

fn index_filter(state: Arc<State>) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path::end()
        .and(with_state(state))
        .map(|state| index(state))
}

/// Start web server
pub async fn start_server(bind: &str) -> anyhow::Result<()> {
    let bind: SocketAddr = bind.parse()?;

    let pool = build_connection_pool()?;
    let state = Arc::new(State { pool });

    let root = index_filter(state);
    let app = root.with(warp::log("bk"));
    warp::serve(app).run(bind).await;
    Ok(())
}
