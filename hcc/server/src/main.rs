#![forbid(unsafe_code)]

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use env_logger::Env;
use log::info;
use serde::Serialize;
use structopt::StructOpt;
use warp::http::StatusCode;
use warp::Filter;

use hcc::{CheckResultJSON, Checker};

#[derive(Debug, StructOpt)]
#[structopt(author, about)]
struct Opts {
    /// host:port to be bound to the server
    #[structopt(short, long, default_value = "127.0.0.1:3000")]
    bind: String,
}

#[derive(Serialize)]
struct ErrorJSON {
    error: String,
}

struct AppState {
    client: Checker,
}

fn with_state(
    state: Arc<AppState>,
) -> impl Filter<Extract = (Arc<AppState>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

#[derive(Debug)]
enum Rejection {
    BadRequest(String),
}

impl warp::reject::Reject for Rejection {}

async fn check_domain_names(
    state: Arc<AppState>,
    domain_names: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let domain_names: Vec<&str> = domain_names.split(',').map(|s| s.trim()).collect();
    let results = match state.client.check_many(domain_names.as_slice()).await {
        Ok(r) => r,
        Err(e) => return Err(warp::reject::custom(Rejection::BadRequest(e.to_string()))),
    };
    if results.len() == 1 {
        let json = CheckResultJSON::new(results.first().unwrap());
        Ok(warp::reply::json(&json))
    } else {
        let json: Vec<CheckResultJSON> = results.iter().map(CheckResultJSON::new).collect();
        Ok(warp::reply::json(&json))
    }
}

fn domain_names_filter(
    state: Arc<AppState>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::get()
        .and(with_state(state))
        .and(warp::path::param::<String>())
        .and_then(|state: Arc<AppState>, domain_names: String| async move {
            check_domain_names(state, domain_names).await
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use warp::http::Method;

    use hcc::Checker;

    use crate::{domain_names_filter, AppState};

    fn build_state() -> Arc<AppState> {
        let mut client = Checker::default();
        client.elapsed = true;
        Arc::new(AppState { client })
    }

    #[tokio::test]
    async fn test_domain_names_filter() {
        let state = build_state();
        let filter = domain_names_filter(state);
        let res = warp::test::request()
            .method(Method::GET.as_str())
            .path("/sha512.badssl.com")
            .reply(&filter)
            .await;
        assert_eq!(200, res.status());
    }
}

async fn handle_rejection(
    err: warp::reject::Rejection,
) -> Result<impl warp::reply::Reply, Infallible> {
    let status_code;
    let error;

    if let Some(Rejection::BadRequest(ref s)) = err.find() {
        status_code = StatusCode::BAD_REQUEST;
        error = s.to_string();
    } else {
        status_code = StatusCode::INTERNAL_SERVER_ERROR;
        error = "unknown error".to_string();
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&ErrorJSON { error }),
        status_code,
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let opts: Opts = Opts::from_args();

    let mut client = Checker::default();
    client.elapsed = true;

    let state = Arc::new(AppState { client });

    let filter = domain_names_filter(state);
    let app = filter
        .recover(handle_rejection)
        .with(warp::log("hcc-server"));

    let bind: SocketAddr = opts.bind.parse()?;
    info!("running on {}", &opts.bind);
    warp::serve(app).bind(bind).await;

    Ok(())
}
