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
use std::thread;
use std::time::Duration;

use clap::Parser;
use cloudflare::framework::response::ApiFailure;
use cron::Schedule;
use env_logger::Env;
use log::{debug, info, warn, Level};
use logging_timer::{finish, timer};

use cdu::{Cdu, NoIPV4};

/// Argument parser
#[derive(Debug, Parser)]
#[clap(about, author, version)]
pub struct Opts {
    /// Cloudflare token
    #[clap(short, long, env = "CLOUDFLARE_TOKEN")]
    pub token: String,
    /// Cloudflare zone name
    #[clap(short, long, env = "CLOUDFLARE_ZONE")]
    pub zone: String,
    /// Cloudflare records separated with comma e.g. a.x.com,b.x.com
    #[clap(short, long, env = "CLOUDFLARE_RECORDS")]
    pub records: String,
    /// Daemon mode
    #[clap(short, long, env = "DAEMON")]
    pub daemon: bool,
    /// Cron. Only in effect in daemon mode
    #[clap(short, long, default_value = "0 */5 * * * * *", env = "CRON")]
    pub cron: String,
    /// Cache duration in seconds, give 0 to disable
    #[clap(short = 's', long, env = "CACHE_SECONDS")]
    pub cache_seconds: Option<u64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::parse();

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
        run_daemon(&mut cdu, &opts.cron).await?;
    } else {
        let tmr = timer!(Level::Debug; "RUN_ONCE", "zone {}", cdu.zone);
        run_once(&mut cdu).await?;
        finish!(tmr);
    }

    Ok(())
}

async fn run_once(cdu: &mut Cdu<'_>) -> anyhow::Result<()> {
    let min = Duration::from_millis(100);
    let max = Duration::from_secs(10);
    let backoff = exponential_backoff::Backoff::new(10, min, max);

    let mut iter = backoff.iter();
    loop {
        let duration = iter.next();
        match cdu.run().await {
            Ok(_) => break,
            Err(e) => {
                if let Some(duration) = duration {
                    if e.is::<ApiFailure>() || e.is::<NoIPV4>() {
                        warn!("retry in {:?} because of {}", duration, e);
                        thread::sleep(duration);
                    } else {
                        return Err(e);
                    }
                } else {
                    return Err(e);
                }
            }
        }
    }

    Ok(())
}

async fn run_daemon<'a, T>(cdu: &mut Cdu<'_>, cron: T) -> anyhow::Result<()>
where
    T: Into<Cow<'a, str>>,
{
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

        run_once(cdu).await?;
    }

    Ok(())
}
