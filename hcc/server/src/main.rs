#![forbid(unsafe_code)]

use log::info;
use serde::Serialize;
use structopt::StructOpt;

use actix_web::{get, middleware, web, App, HttpResponse, HttpServer};
use env_logger::Env;
use hcc::{CheckClient, CheckResultJSON};

#[derive(Debug, StructOpt)]
#[structopt(author, about)]
struct Opts {
    /// host:port to be bound to the server
    #[structopt(short, long, default_value = "127.0.0.1:9292")]
    bind: String,
}

#[derive(Serialize)]
struct ErrorMessage {
    message: String,
}

struct AppState {
    client: CheckClient,
}

#[get("/{domain_names}")]
async fn show_domain_name(
    data: web::Data<AppState>,
    web::Path((domain_names,)): web::Path<(String,)>,
) -> HttpResponse {
    let domain_names: Vec<&str> = domain_names.split(',').map(|s| s.trim()).collect();
    let results = match data.client.check_certificates(domain_names.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::InternalServerError().json(&ErrorMessage {
                message: format!("{:?}", e),
            });
        }
    };
    if results.len() == 1 {
        let json = CheckResultJSON::new(results.first().unwrap());
        HttpResponse::Ok().json(&json)
    } else {
        let json: Vec<CheckResultJSON> = results.iter().map(|r| CheckResultJSON::new(&r)).collect();
        HttpResponse::Ok().json(&json)
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let opts: Opts = Opts::from_args();
    let data = web::Data::new(AppState {
        client: CheckClient::builder().elapsed(true).build(),
    });

    info!("Served on {0}", &opts.bind);
    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::Logger::default())
            .service(show_domain_name)
    })
    .bind(&opts.bind)?
    .run()
    .await?;

    Ok(())
}
