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

use actix_web::{middleware, post, web, App, HttpRequest, HttpResponse, HttpServer};
use env_logger::Env;
use log::warn;
use pushover::{Attachment, Notification, Priority, Sound, HTML};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use structopt::StructOpt;

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

struct AppState {
    authorization: Option<String>,
    token: String,
    user: String,
}

#[derive(Serialize)]
struct ErrorMessage {
    message: String,
}

fn get_authorization(req: &HttpRequest) -> Option<&str> {
    req.headers().get("authorization")?.to_str().ok()
}

#[post("/1/messages")]
async fn messages(
    req: HttpRequest,
    data: web::Data<AppState>,
    message: web::Json<Message>,
) -> HttpResponse {
    if let Some(ref expected) = data.authorization {
        if let Some(actual) = get_authorization(&req) {
            if expected != actual {
                // authorization and header are present but not equal
                return HttpResponse::BadRequest().json(&ErrorMessage {
                    message: "unauthorized".to_string(),
                });
            } else {
                // authorization and header are present and equal
            }
        } else {
            // authorization is present but header is absent
            return HttpResponse::BadRequest().json(&ErrorMessage {
                message: "unauthorized".to_string(),
            });
        }
    } else {
        // authorization is absent
    }

    let mut n = Notification::new(&data.token, &data.user, &message.message);

    if let Some(ref d) = message.device {
        n.request.device = Some(d.into());
    }
    if let Some(ref t) = message.title {
        n.request.title = Some(t.into());
    }
    if let Some(ref h) = message.html {
        n.request.html = Some(match HTML::from_str(h) {
            Ok(h) => h,
            Err(e) => return bad_request(e),
        });
    }

    n.request.timestamp = message.timestamp;

    if let Some(ref p) = message.priority {
        n.request.priority = Some(match Priority::from_str(p) {
            Ok(p) => p,
            Err(e) => return bad_request(e),
        });
    }
    if let Some(ref u) = message.url {
        n.request.url = Some(u.into());
        if let Some(ref t) = message.url_title {
            n.request.url_title = Some(t.into());
        }
    }
    if let Some(ref s) = message.sound {
        n.request.sound = Some(match Sound::from_str(s) {
            Ok(s) => s,
            Err(e) => return bad_request(e),
        });
    }

    let attachment;
    if let Some(ref url) = message.image_url {
        attachment = match Attachment::from_url(url).await {
            Ok(a) => a,
            Err(e) => return bad_request(e),
        };
        n.attach(&attachment);
    }

    let response = match n.send().await {
        Ok(r) => r,
        Err(e) => return bad_request(e),
    };

    if 1 == response.status {
        HttpResponse::Ok().json(&response)
    } else {
        HttpResponse::BadRequest().json(&response)
    }
}

fn bad_request<S: std::fmt::Debug>(e: S) -> HttpResponse {
    HttpResponse::BadRequest().body(format!("{:?}", e))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let opts: Opts = Opts::from_args();
    if opts.authorization.is_none() {
        warn!("no authorization set, server is vulnerable");
    }

    let data = web::Data::new(AppState {
        authorization: opts.authorization,
        token: opts.token,
        user: opts.user,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::Logger::default())
            .service(messages)
    })
    .bind(&opts.bind)?
    .run()
    .await?;

    Ok(())
}
