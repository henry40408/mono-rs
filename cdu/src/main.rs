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
use log::{debug, info, warn, Level};
use logging_timer::{finish, timer};

use cdu::{Cdu, NoIPV4};

/// Argument parser
#[derive(Debug, Parser)]
#[command(about, author, version)]
pub struct Opts {
    /// Cloudflare token
    #[arg(short, long, env = "CLOUDFLARE_TOKEN")]
    pub token: String,
    /// Cloudflare zone name
    #[arg(short, long, env = "CLOUDFLARE_ZONE")]
    pub zone: String,
    /// Cloudflare records separated with comma e.g. a.x.com,b.x.com
    #[arg(short, long, env = "CLOUDFLARE_RECORDS")]
    pub records: String,
    /// Daemon mode
    #[arg(short, long, env = "DAEMON", action = clap::ArgAction::SetTrue)]
    pub daemon: bool,
    /// Cron. Only in effect in daemon mode
    #[arg(short, long, default_value = "0 */5 * * * * *", env = "CRON")]
    pub cron: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opts: Opts = Opts::parse();

    let record_names = opts
        .records
        .split(',')
        .map(String::from)
        .collect::<Vec<String>>();

    let cdu = Cdu::new(&opts.token, &opts.zone, &record_names);

    if opts.daemon {
        let cron = &opts.cron;
        debug!("run as daemon with cron {cron}");
        run_daemon(&cdu, cron).await?;
    } else {
        let zone = &opts.zone;
        let tmr = timer!(Level::Debug; "RUN_ONCE", "zone {zone}");
        run_once(&cdu).await?;
        finish!(tmr);
    }

    Ok(())
}

async fn run_once(cdu: &Cdu<'_>) -> anyhow::Result<()> {
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
                        warn!("retry in {duration:?} because of {e}");
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

async fn run_daemon<'a, T>(cdu: &Cdu<'_>, cron: T) -> anyhow::Result<()>
where
    T: Into<Cow<'a, str>>,
{
    let schedule = Schedule::from_str(cron.into().as_ref())?;
    for datetime in schedule.upcoming(chrono::Utc) {
        info!("update DNS records at {datetime}");

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_daemon_mode() {
        let opts = Opts::try_parse_from(vec![
            "--", "-t", "token", "-z", "zone", "-r", "records", "--daemon",
        ])
        .unwrap();
        assert!(opts.daemon);
        assert_eq!(opts.records, "records");
        assert_eq!(opts.token, "token");
        assert_eq!(opts.zone, "zone");
    }
}
