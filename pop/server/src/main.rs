#![forbid(unsafe_code)]

use actix_web::{middleware, post, web, App, HttpRequest, HttpResponse, HttpServer};
use env_logger::Env;
use log::warn;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use pop::notification::Notification;

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
    html: Option<u8>,
    timestamp: Option<u64>,
    priority: Option<u8>,
    url: Option<String>,
    url_title: Option<String>,
    sound: Option<String>,
    image_url: Option<String>,
}

impl Message {
    async fn to_notification(&self, token: &str, user: &str) -> anyhow::Result<Notification> {
        let mut n = Notification::new(token, user, &self.message);

        n.request.device = self.device.clone();
        n.request.title = self.title.clone();
        n.request.html = self.html;
        n.request.timestamp = self.timestamp;
        n.request.priority = self.priority;
        n.request.url = self.url.clone();
        n.request.url_title = self.url_title.clone();
        n.request.sound = self.sound.clone();

        if let Some(ref url) = self.image_url {
            n.attach_url(url).await
        } else {
            Ok(n)
        }
    }
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

#[post("/v1/messages")]
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

    let notification = match message.to_notification(&data.token, &data.user).await {
        Ok(n) => n,
        Err(e) => return HttpResponse::BadRequest().body(format!("{:?}", e)),
    };

    let response = match notification.send().await {
        Ok(r) => r,
        Err(e) => return HttpResponse::BadRequest().body(format!("{:?}", e)),
    };

    if 1 == response.status {
        HttpResponse::Ok().json(&response)
    } else {
        HttpResponse::BadRequest().json(&response)
    }
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
