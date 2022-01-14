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

//! Daemon to send check result to Pushover

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use chrono::Utc;
use cron::Schedule;
use env_logger::Env;
use log::info;
use structopt::StructOpt;

use hcc::Checker;
use pushover::Notification;

#[derive(Debug, StructOpt)]
#[structopt(author, about)]
struct Opts {
    /// Domain names to check, separated by comma e.g. sha512.badssl.com,expired.badssl.com
    #[structopt(short, long, env = "DOMAIN_NAMES")]
    domain_names: String,
    /// Cron
    #[structopt(short, long, env = "CRON", default_value = "0 */5 * * * * *")]
    cron: String,
    /// Pushover API key
    #[structopt(short = "t", long = "token", env = "PUSHOVER_TOKEN")]
    pushover_token: String,
    /// Pushover user key,
    #[structopt(short = "u", long = "user", env = "PUSHOVER_USER")]
    pushover_user: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let opts: Opts = Opts::from_args();
    let schedule = Schedule::from_str(&opts.cron)?;

    info!("check HTTPS certificates with cron {}", &opts.cron);
    for datetime in schedule.upcoming(Utc) {
        info!("check certificate of {} at {}", opts.domain_names, datetime);
        loop {
            if Utc::now() > datetime {
                break;
            } else {
                tokio::time::sleep(Duration::from_millis(999)).await;
            }
        }
        let start = Instant::now();
        let domain_names: Vec<_> = opts.domain_names.split(',').collect();
        check_domain_names(&opts, &domain_names).await?;
        let elapsed = start.elapsed();
        info!("done in {}ms", elapsed.as_millis());
    }

    Ok(())
}

async fn check_domain_names(opts: &Opts, domain_names: &[&str]) -> anyhow::Result<()> {
    let check_client = Checker::default();
    let results = check_client.check_many(domain_names).await?;

    let mut tasks = vec![];

    for result in results {
        let r = Arc::new(result);
        tasks.push(async move {
            let title = format!("HTTP Certificate Check - {}", r.domain_name);

            let state_icon = r.state_icon(true);
            let sentence = r.sentence();
            let message = format!("{} {}", state_icon, sentence);

            let mut n = Notification::new(&opts.pushover_token, &opts.pushover_user, &message);
            n.title = Some(&title);
            n.send().await?;

            Ok::<(), anyhow::Error>(())
        });
    }

    futures::future::try_join_all(tasks).await?;

    Ok(())
}
