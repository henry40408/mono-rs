#![forbid(unsafe_code)]

use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cloudflare::framework::response::ApiFailure;
use cron::Schedule;
use log::info;
use structopt::StructOpt;
use tokio_retry::strategy::{jitter, ExponentialBackoff};

use cdu::{Cdu, Opts, PublicIPError};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::from_args();

    let cdu = Cdu::new(opts);
    if env::var_os("RUST_LOG").is_none() {
        if cdu.is_debug() {
            env::set_var("RUST_LOG", "cdu=debug");
        } else {
            env::set_var("RUST_LOG", "cdu=info");
        }
    }

    pretty_env_logger::init();

    if cdu.is_daemon() {
        run_daemon(cdu).await?;
    } else {
        cdu.run().await?;
    }

    Ok(())
}

async fn run_daemon(cdu: Cdu) -> anyhow::Result<()> {
    let cdu = Arc::new(cdu);
    let schedule = Schedule::from_str(cdu.cron())?;
    for datetime in schedule.upcoming(chrono::Utc) {
        info!("update DNS records at {}", datetime);

        loop {
            if chrono::Utc::now() > datetime {
                break;
            } else {
                tokio::time::sleep(Duration::from_millis(999)).await;
            }
        }

        let strategy = ExponentialBackoff::from_millis(10).map(jitter).take(3);
        let cdu = cdu.clone();
        let instant = Instant::now();
        tokio_retry::RetryIf::spawn(
            strategy,
            || cdu.run(),
            |e: &anyhow::Error| e.is::<ApiFailure>() || e.is::<PublicIPError>(),
        )
        .await?;
        let duration = Instant::now() - instant;
        info!("done in {}ms", duration.as_millis());
    }

    Ok(())
}
