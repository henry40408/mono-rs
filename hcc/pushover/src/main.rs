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

use chrono::Utc;
use clap::Parser;
use cron::Schedule;
use env_logger::Env;
use log::{info, Level};
use logging_timer::{finish, stimer};

use hcc::Checker;
use pushover::Notification;

#[derive(Debug, Parser)]
#[clap(author, about, version)]
struct Opts {
    /// Domain names to check, separated by comma e.g. sha256.badssl.com,expired.badssl.com
    #[clap(short, long, env = "DOMAIN_NAMES")]
    domain_names: String,
    /// Cron
    #[clap(short, long, env = "CRON", default_value = "0 */5 * * * * *")]
    cron: String,
    /// Pushover API key
    #[clap(short = 't', long = "token", env = "PUSHOVER_TOKEN")]
    pushover_token: String,
    /// Pushover user key,
    #[clap(short = 'u', long = "user", env = "PUSHOVER_USER")]
    pushover_user: String,
    /// Run immediately
    #[clap(long = "run-immediately", env = "RUN_IMMEDIATELY")]
    run_immediately: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let opts: Opts = Opts::parse();
    let schedule = Schedule::from_str(&opts.cron)?;

    info!("check HTTPS certificates with cron {}", &opts.cron);
    for datetime in schedule.upcoming(Utc) {
        let domain_names: Vec<_> = opts.domain_names.split(',').collect();
        if opts.run_immediately {
            info!("run immediately");
            check_domain_names(&opts, &domain_names).await?;
        }
        info!("check certificate of {} at {}", opts.domain_names, datetime);
        loop {
            if Utc::now() > datetime {
                break;
            } else {
                tokio::time::sleep(Duration::from_millis(999)).await;
            }
        }
        check_domain_names(&opts, &domain_names).await?;
    }

    Ok(())
}

async fn check_domain_names(opts: &Opts, domain_names: &[&str]) -> anyhow::Result<()> {
    let check_client = Checker::default();
    let tmr = stimer!("CHECK_CERT");
    let results = check_client.check_many(domain_names).await;
    finish!(tmr);

    let mut tasks = vec![];
    for (index, result) in results.iter().enumerate() {
        let r = Arc::new(result);
        let domain_name = domain_names[index].to_string();
        tasks.push(async move {
            let title = format!("HTTP Certificate Check - {domain_name}");

            let state_icon = r.state_icon();
            let sentence = r.sentence();
            let message = format!("{state_icon} {sentence}");

            let mut n = Notification::new(&opts.pushover_token, &opts.pushover_user, &message);
            n.title = Some(&title);

            let tmr = stimer!(Level::Debug; "NOTIFY");
            n.send().await?;
            finish!(tmr);

            Ok::<(), anyhow::Error>(())
        });
    }

    futures::future::try_join_all(tasks).await?;

    Ok(())
}
