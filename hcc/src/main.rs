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

use std::borrow::Cow;

use clap::Parser;

use hcc::Checker;
use pushover::{send_notification, NotificationError};

#[derive(Debug, Default, Parser)]
#[clap(author, about, version)]
struct Opts {
    /// ASCII
    #[clap(long)]
    ascii: bool,
    /// Verbose mode
    #[clap(short, long)]
    verbose: bool,
    /// Pushover token
    #[clap(long, env = "PUSHOVER_TOKEN")]
    pushover_token: Option<String>,
    /// Pushover user
    #[clap(long, env = "PUSHOVER_USER")]
    pushover_user: Option<String>,
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Parser)]
enum Commands {
    /// Check domain name(s) immediately
    #[clap()]
    Check {
        /// Grace period in days
        #[clap(short, long = "grace", default_value = "7")]
        grace_in_days: i64,
        /// One or many domain names to check
        #[clap()]
        domain_names: Vec<String>,
    },
    /// Daemon
    #[clap()]
    Daemon {
        /// Cron
        #[clap(short, long, default_value = "0 0 * * *")]
        cron: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::parse();
    if let Some(Commands::Check {
        ref domain_names,
        grace_in_days,
    }) = opts.command
    {
        let domain_names: Vec<&str> = domain_names.iter().map(AsRef::as_ref).collect();
        check_command(&opts, &domain_names, grace_in_days).await?;
    }
    Ok(())
}

async fn check_command<T>(opts: &Opts, domain_names: &[T], grace_in_days: i64) -> anyhow::Result<()>
where
    T: AsRef<str>,
{
    let mut client = Checker::default();
    client.ascii = opts.ascii;
    client.elapsed = opts.verbose;
    client.grace_in_days = grace_in_days;

    let results = client.check_many(domain_names).await;
    let mut tasks = vec![];
    for result in results.iter() {
        println!("{}", result);
        tasks.push(notify(opts, result.to_string()));
    }
    futures::future::join_all(tasks).await;
    Ok(())
}

fn get_pushover_config(opts: &'_ Opts) -> Option<(Cow<'_, str>, Cow<'_, str>)> {
    let t = opts.pushover_token.as_ref()?;
    let u = opts.pushover_user.as_ref()?;
    Some((t.into(), u.into()))
}

async fn notify<'a, T>(opts: &Opts, message: T) -> Result<(), NotificationError>
where
    T: Into<Cow<'a, str>>,
{
    let (token, user) = match get_pushover_config(opts) {
        Some((t, u)) => (t, u),
        None => return Ok(()),
    };
    send_notification(token, user, message.into()).await?;
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
        check_command(&opts, &["sha256.badssl.com"], 7)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn t_check_command_expired() {
        let opts = build_opts();
        check_command(&opts, &["expired.badssl.com"], 7)
            .await
            .unwrap();
    }
}
