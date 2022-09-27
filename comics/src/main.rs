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

//! comics is a simple comics server

use std::net::SocketAddr;

use clap::Parser;
use warp::Filter;

#[derive(Parser)]
#[clap(about, author, version)]
struct Opts {
    /// Bind host and port
    #[clap(short, long, default_value = "127.0.0.1:3000")]
    bind: String,
    /// Data directory
    #[clap(short, long, default_value = "./data")]
    data_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opts = Opts::parse();

    let static_route = warp::path("static").and(warp::fs::dir(opts.data_dir));

    let router = static_route;

    let bind: SocketAddr = opts.bind.parse()?;
    warp::serve(router).run(bind).await;

    Ok(())
}
