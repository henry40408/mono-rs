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
use std::str::FromStr;
use std::sync::Arc;

use env_logger::Env;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use warp::http::StatusCode;
use warp::Filter;

use pushover::{Attachment, Notification, Priority, Sound, HTML};

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

#[derive(Debug)]
enum Rejection {
    Unauthorized,
    BadRequest(String),
    Parse(String),
}

impl warp::reject::Reject for Rejection {}

async fn send_notification(
    opts: Arc<Opts>,
    actual: Option<String>,
    message: &Message,
) -> Result<warp::reply::Json, warp::Rejection> {
    match check_authorization(&opts.authorization, &actual) {
        AuthorizationState::Rejected => Err(warp::reject::custom(Rejection::Unauthorized)),
        AuthorizationState::Public | AuthorizationState::Accepted => {
            let mut n = Notification::new(&opts.token, &opts.user, &message.message);

            if let Some(ref d) = message.device {
                n.request.device = Some(d);
            }

            if let Some(ref t) = message.title {
                n.request.title = Some(t);
            }

            if let Some(ref h) = message.html {
                n.request.html = Some(match HTML::from_str(h) {
                    Ok(h) => h,
                    Err(e) => {
                        return Err(warp::reject::custom(Rejection::Parse(e.to_string())));
                    }
                });
            }

            if let Some(ref t) = message.timestamp {
                n.request.timestamp = Some(*t);
            }

            if let Some(ref p) = message.priority {
                n.request.priority = Some(match Priority::from_str(p) {
                    Ok(p) => p,
                    Err(e) => {
                        let e = e.to_string();
                        return Err(warp::reject::custom(Rejection::Parse(e)));
                    }
                })
            }

            if let Some(ref u) = message.url {
                n.request.url = Some(u);
                if let Some(ref t) = message.url_title {
                    n.request.url_title = Some(t);
                }
            }

            if let Some(ref s) = message.sound {
                n.request.sound = Some(match Sound::from_str(s) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(warp::reject::custom(Rejection::Parse(e.to_string())));
                    }
                });
            }

            let a;
            if let Some(ref u) = message.image_url {
                a = match Attachment::from_url(u).await {
                    Ok(a) => a,
                    Err(e) => {
                        return Err(warp::reject::custom(Rejection::BadRequest(e.to_string())));
                    }
                };
                n.attachment = Some(&a);
            }

            match n.send().await {
                Ok(r) => Ok(warp::reply::json(&r)),
                Err(e) => Err(warp::reject::custom(Rejection::BadRequest(e.to_string()))),
            }
        }
    }
}

fn with_opts(opts: Arc<Opts>) -> impl Filter<Extract = (Arc<Opts>,), Error = Infallible> + Clone {
    warp::any().map(move || opts.clone())
}

fn one_messages_filter(
    opts: Arc<Opts>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
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
            token: "".into(),
            user: "".into(),
            authorization: None,
            bind: "".into(),
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

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::reply::Reply, Infallible> {
    let status_code;
    let error;

    if let Some(Rejection::Unauthorized) = err.find() {
        status_code = StatusCode::UNAUTHORIZED;
        error = "unauthorized".into();
    } else if let Some(Rejection::BadRequest(ref s)) = err.find() {
        status_code = StatusCode::BAD_REQUEST;
        error = s.into();
    } else {
        eprintln!("{:?}", err);
        status_code = StatusCode::INTERNAL_SERVER_ERROR;
        error = "unknown error".into();
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&ErrorJson { error }),
        status_code,
    ))
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
    let app = filter.recover(handle_rejection).with(warp::log("pop"));

    let bind: SocketAddr = opts.bind.parse()?;
    info!("running on {}", opts.bind);
    warp::serve(app).bind(bind).await;

    Ok(())
}
