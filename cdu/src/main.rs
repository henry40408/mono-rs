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

//! Cloudflare DNS record update

use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cloudflare::framework::response::ApiFailure;
use cron::Schedule;
use env_logger::Env;
use log::{debug, info};
use structopt::StructOpt;
use tokio_retry::strategy::{jitter, ExponentialBackoff};

use cdu::{Cdu, RecoverableError};

/// Argument parser
#[derive(Debug, StructOpt)]
#[structopt(about, author)]
pub struct Opts {
    /// Cloudflare token
    #[structopt(short, long, env = "CLOUDFLARE_TOKEN")]
    pub token: String,
    /// Cloudflare zone name
    #[structopt(short, long, env = "CLOUDFLARE_ZONE")]
    pub zone: String,
    /// Cloudflare records separated with comma e.g. a.x.com,b.x.com
    #[structopt(short, long, env = "CLOUDFLARE_RECORDS")]
    pub records: String,
    /// Daemon mode
    #[structopt(short, long, env = "DAEMON")]
    pub daemon: bool,
    /// Cron. Only in effect in daemon mode
    #[structopt(short, long, default_value = "0 */5 * * * * *", env = "CRON")]
    pub cron: String,
    /// Cache duration in seconds, give 0 to disable
    #[structopt(short = "s", long, env = "CACHE_SECONDS")]
    pub cache_seconds: Option<u64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::from_args();

    let record_names = opts
        .records
        .split(',')
        .map(String::from)
        .collect::<Vec<String>>();

    let mut cdu = Cdu::new(&opts.token, &opts.zone, &record_names);
    cdu.cache_seconds = opts.cache_seconds;

    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    if opts.daemon {
        debug!("run as daemon with cron {}", opts.cron);
        run_daemon(cdu, &opts.cron).await?;
    } else {
        debug!("run once");
        cdu.run().await?;
    }

    Ok(())
}

async fn run_daemon<'a, T>(cdu: Cdu<'_>, cron: T) -> anyhow::Result<()>
where
    T: Into<Cow<'a, str>>,
{
    let cdu = Arc::new(cdu);
    let schedule = Schedule::from_str(cron.into().as_ref())?;
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
        let start = Instant::now();
        tokio_retry::RetryIf::spawn(
            strategy,
            || cdu.run(),
            |e: &anyhow::Error| e.is::<ApiFailure>() || e.is::<RecoverableError>(),
        )
        .await?;
        let elapsed = start.elapsed();
        info!("done in {}ms", elapsed.as_millis());
    }

    Ok(())
}
