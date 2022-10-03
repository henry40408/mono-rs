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

//! HTTPS Certificate Check

use std::{borrow::Cow, time::Duration};

use chrono::Utc;
use clap::{Parser, Subcommand};

use cron::Schedule;
use hcc::Checker;
use log::debug;
use pushover::{send_notification, NotificationError};

#[derive(Debug, Default, Parser)]
#[command(author, about, version)]
struct Opts {
    /// ASCII
    #[arg(long)]
    ascii: bool,
    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,
    /// Grace period in days
    #[arg(short, long = "grace", default_value = "7")]
    grace_in_days: i64,
    /// Pushover token
    #[arg(long, env = "PUSHOVER_TOKEN")]
    pushover_token: Option<String>,
    /// Pushover user
    #[arg(long, env = "PUSHOVER_USER")]
    pushover_user: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check domain name(s) immediately
    Check {
        /// Send notification
        #[arg(long)]
        notify: bool,
        /// One or many domain names to check
        #[arg()]
        domain_names: Vec<String>,
    },
    /// Daemon
    Daemon {
        /// Cron
        #[arg(short, long, default_value = "0 0 0 * * *")]
        cron: String,
        /// One or many domain names to check
        #[arg(env = "DOMAIN_NAMES")]
        domain_names: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opts: Opts = Opts::parse();
    if let Some(Commands::Check {
        domain_names,
        notify,
    }) = &opts.command
    {
        check_command(&opts, domain_names, *notify).await?;
    }
    if let Some(Commands::Daemon { cron, domain_names }) = &opts.command {
        daemon_command(&opts, cron, domain_names).await?;
    }
    Ok(())
}

async fn check_command<T>(
    opts: &Opts,
    domain_names: &[T],
    should_notify: bool,
) -> anyhow::Result<()>
where
    T: AsRef<str>,
{
    let mut client = Checker::default();
    client.ascii = opts.ascii;
    client.elapsed = opts.verbose;
    client.grace_in_days = opts.grace_in_days;

    let results = client.check_many(domain_names).await;
    let mut tasks = vec![];
    for result in results.iter() {
        println!("{result}");
        if should_notify {
            tasks.push(notify(opts, result.to_string()));
        }
    }
    futures::future::join_all(tasks).await;
    Ok(())
}

async fn daemon_command<'a, T, U>(opts: &Opts, cron: T, domain_names: &[U]) -> anyhow::Result<()>
where
    T: AsRef<str>,
    U: AsRef<str> + std::fmt::Debug,
{
    use std::str::FromStr as _;

    let mut client = Checker::default();
    client.ascii = opts.ascii;
    client.elapsed = opts.verbose;
    client.grace_in_days = opts.grace_in_days;

    let cron = cron.as_ref();
    let schedule = Schedule::from_str(cron)?;
    for next in schedule.upcoming(Utc) {
        debug!("check certificates of {domain_names:?} at {next:?}");
        loop {
            if Utc::now().timestamp() >= next.timestamp() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(999)).await;
        }

        debug!("check {domain_names:?}");
        let results = client.check_many(domain_names).await;
        let mut tasks = vec![];
        for result in results.iter() {
            debug!("{result}");
            tasks.push(notify(opts, result.to_string()));
        }
        futures::future::join_all(tasks).await;
    }
    Ok(())
}

fn get_pushover_config(opts: &'_ Opts) -> Option<(Cow<'_, str>, Cow<'_, str>)> {
    let t = opts.pushover_token.as_ref()?;
    let u = opts.pushover_user.as_ref()?;
    Some((t.into(), u.into()))
}

async fn notify<'a, T>(opts: &Opts, message: T) -> Result<(), NotificationError>
where
    T: Into<Cow<'a, str>> + std::fmt::Debug,
{
    let (token, user) = match get_pushover_config(opts) {
        Some((t, u)) => (t, u),
        None => return Ok(()),
    };
    debug!("send pushover notification {:?}", message);
    let res = send_notification(token, user, message.into()).await?;
    debug!("pushover response {:?}", res);
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    fn build_opts() -> Opts {
        Opts::default()
    }

    #[tokio::test]
    async fn t_check_command() {
        let opts = build_opts();
        check_command(&opts, &["sha256.badssl.com"], false)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn t_check_command_expired() {
        let opts = build_opts();
        check_command(&opts, &["expired.badssl.com"], false)
            .await
            .unwrap();
    }
}
