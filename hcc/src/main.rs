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

use std::fmt::Display;
use std::{borrow::Cow, time::Duration};

use chrono::Utc;
use clap::{Parser, Subcommand};
use cron::Schedule;
use futures::stream::FuturesUnordered;
use hcc::{Checked, CheckedInner, Checker};
use log::debug;
use once_cell::sync::OnceCell;
use pushover::{send_notification, NotificationError};
use supports_unicode::Stream;

fn get_opts() -> &'static Opts {
    static INSTANCE: OnceCell<Opts> = OnceCell::new();
    INSTANCE.get_or_init(Opts::parse)
}

#[derive(Debug, Default, Parser)]
#[command(author, about, version)]
struct Opts {
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

struct CheckedString<'a> {
    inner: &'a Checked<'a>,
    grace_in_days: i64,
}

impl<'a> Display for CheckedString<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_unicode = supports_unicode::on(Stream::Stdout);
        let domain_name = &self.inner.domain_name;
        let grace = chrono::Duration::days(self.grace_in_days);
        match &self.inner.inner {
            CheckedInner::Ok { not_after, .. } => {
                if not_after > &(self.inner.checked_at + grace) {
                    let icon = if is_unicode { "\u{2705}" } else { "[v]" };
                    write!(f, "{icon} {domain_name} expires at {not_after}")
                } else if not_after > &self.inner.checked_at {
                    let icon = if is_unicode {
                        "\u{26a0}\u{fe0f}"
                    } else {
                        "[!]"
                    };
                    let duration = *not_after - self.inner.checked_at;
                    let days = duration.num_days();
                    write!(
                        f,
                        "{icon} {domain_name} expires in {days} day(s) at {not_after}"
                    )
                } else {
                    let icon = if is_unicode { "\u{274c}" } else { "[x]" };
                    write!(f, "{icon} {domain_name} expired at {not_after}")
                }
            }
            CheckedInner::Error { error } => {
                let icon = if is_unicode { "\u{274c}" } else { "[x]" };
                write!(f, "{icon} {domain_name}: {error}")
            }
        }
    }
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
    use futures::StreamExt as _;

    let client = Checker::default();
    let results = client.check_many(domain_names).await?;

    let mut tasks = FuturesUnordered::new();
    for result in results.iter() {
        let result = CheckedString {
            inner: result,
            grace_in_days: opts.grace_in_days,
        }
        .to_string();
        println!("{result}");
        if should_notify {
            tasks.push(tokio::spawn(async move { notify(result).await }));
        }
    }

    while let Some(task) = tasks.next().await {
        task??;
    }

    Ok(())
}

async fn daemon_command<'a, T, U>(opts: &Opts, cron: T, domain_names: &[U]) -> anyhow::Result<()>
where
    T: AsRef<str>,
    U: AsRef<str> + std::fmt::Debug,
{
    use futures::StreamExt as _;
    use std::str::FromStr as _;

    let client = Checker::default();

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
        let results = client.check_many(domain_names).await?;

        let mut tasks = FuturesUnordered::new();
        for result in results.iter() {
            let result = CheckedString {
                inner: result,
                grace_in_days: opts.grace_in_days,
            }
            .to_string();
            debug!("{result}");
            tasks.push(tokio::spawn(async move { notify(result).await }));
        }

        while let Some(task) = tasks.next().await {
            task??;
        }
    }

    Ok(())
}

fn get_pushover_config<'a>() -> Option<(Cow<'a, str>, Cow<'a, str>)> {
    let opts = get_opts();
    let t = opts.pushover_token.as_ref()?;
    let u = opts.pushover_user.as_ref()?;
    Some((t.into(), u.into()))
}

async fn notify<'a, T>(message: T) -> Result<(), NotificationError>
where
    T: Into<Cow<'a, str>>,
{
    let message = message.into();
    let (token, user) = match get_pushover_config() {
        Some((t, u)) => (t, u),
        None => return Ok(()),
    };
    debug!("send pushover notification {message:?}");
    let res = send_notification(token, user, message).await?;
    debug!("pushover response {res:?}");
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
