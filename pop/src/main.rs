#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

//! Pop is proxy server to Pushover with attachment support

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use env_logger::Env;
use log::warn;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use warp::http::StatusCode;
use warp::Filter;

use pushover::{Attachment, Notification, Priority, Response, Sound, HTML};

#[derive(StructOpt)]
#[structopt(about, author)]
struct Opts {
    /// Pushover API token
    #[structopt(short, long, env = "PUSHOVER_TOKEN")]
    token: String,
    /// Pushover user token
    #[structopt(short, long, env = "PUSHOVER_USER")]
    user: String,
    /// Authorization token to protect the proxy
    #[structopt(short, long, env = "AUTHORIZATION")]
    authorization: Option<String>,
    /// host and port to bind
    #[structopt(short, long, env = "BIND", default_value = "127.0.0.1:3000")]
    bind: String,
}

#[derive(Default, Debug, Deserialize, Serialize)]
struct Message {
    device: Option<String>,
    title: Option<String>,
    message: String,
    html: Option<String>,
    timestamp: Option<u64>,
    priority: Option<String>,
    url: Option<String>,
    url_title: Option<String>,
    sound: Option<String>,
    image_url: Option<String>,
}

#[derive(Serialize)]
struct ErrorJson {
    error: String,
}

enum AuthorizationState {
    Public,
    Accepted,
    Rejected,
}

fn check_authorization(expected: &Option<String>, actual: &Option<String>) -> AuthorizationState {
    if let Some(e) = expected {
        if let Some(a) = actual {
            if a == e {
                AuthorizationState::Accepted
            } else {
                AuthorizationState::Rejected
            }
        } else {
            AuthorizationState::Rejected
        }
    } else {
        AuthorizationState::Public
    }
}

async fn send_notification(
    opts: Arc<Opts>,
    actual: Option<String>,
    message: &Message,
) -> Result<warp::reply::WithStatus<warp::reply::Json>, Infallible> {
    match check_authorization(&opts.authorization, &actual) {
        AuthorizationState::Rejected => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorJson {
                error: "unauthorized".to_string(),
            }),
            StatusCode::UNAUTHORIZED,
        )),
        AuthorizationState::Public | AuthorizationState::Accepted => {
            let n = Notification::new(&opts.token, &opts.user, &message.message);
            match n.send().await {
                Ok(r) => Ok(warp::reply::with_status(
                    warp::reply::json(&r),
                    StatusCode::OK,
                )),
                Err(e) => Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorJson {
                        error: format!("{:?}", e),
                    }),
                    StatusCode::BAD_REQUEST,
                )),
            }
        }
    }
}

fn with_opts(opts: Arc<Opts>) -> impl Filter<Extract = (Arc<Opts>,), Error = Infallible> + Clone {
    warp::any().map(move || opts.clone())
}

fn one_messages_filter(
    opts: Arc<Opts>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::reject::Rejection> + Clone {
    warp::post()
        .and(warp::path("1"))
        .and(warp::path("messages"))
        .and(with_opts(opts))
        .and(warp::body::json())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(
            |opts: Arc<Opts>, message: Message, actual: Option<String>| async move {
                send_notification(opts.clone(), actual, &message).await
            },
        )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mockito::mock;
    use warp::http::Method;

    use crate::one_messages_filter;

    use super::Opts;

    #[tokio::test]
    async fn test_one_messages_filter() {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"647d2300-702c-4b38-8b2f-d56326ae460b"}"#)
            .create();
        let opts = Arc::new(Opts {
            token: "".to_string(),
            user: "".to_string(),
            authorization: None,
            bind: "".to_string(),
        });
        let filter = one_messages_filter(opts);
        let res = warp::test::request()
            .method(Method::POST.as_str())
            .path("/1/messages")
            .body(r#"{"message":"test"}"#)
            .reply(&filter)
            .await;
        assert_eq!(200, res.status());
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let opts: Opts = Opts::from_args();
    if opts.authorization.is_none() {
        warn!("no authorization set, server is vulnerable");
    }

    let opts = Arc::new(opts);
    let filter = one_messages_filter(opts.clone());
    let app = filter.with(warp::log("pop"));

    let bind: SocketAddr = opts.bind.parse()?;
    warp::serve(app).bind(bind).await;

    Ok(())
}
