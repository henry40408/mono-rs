#![forbid(unsafe_code)]
use std::env;
use std::str::FromStr;
use std::time::Duration;
use std::time::Instant;

use chrono::Utc;
use cron::Schedule;
use log::info;
use structopt::StructOpt;

use hcc::CheckClient;

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

const PUSHOVER_API: &str = "https://api.pushover.net/1/messages.json";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "hcc_pushover=info");
    }

    pretty_env_logger::init();

    let opts: Opts = Opts::from_args();
    let schedule = Schedule::from_str(&opts.cron)?;

    info!("check HTTPS certficates with cron {}", &opts.cron);
    for datetime in schedule.upcoming(Utc) {
        info!("check certificate of {} at {}", opts.domain_names, datetime);
        loop {
            if Utc::now() > datetime {
                break;
            } else {
                tokio::time::sleep(Duration::from_millis(999)).await;
            }
        }
        let instant = Instant::now();
        let domain_names: Vec<_> = opts.domain_names.split(',').collect();
        check_domain_names(&opts, &domain_names).await?;
        let duration = Instant::now() - instant;
        info!("done in {}ms", duration.as_millis());
    }

    Ok(())
}

async fn check_domain_names(opts: &Opts, domain_names: &[&str]) -> anyhow::Result<()> {
    let check_client = CheckClient::new();
    let results = check_client.check_certificates(domain_names)?;

    let mut futs = vec![];

    let pushover_client = reqwest::Client::new();
    for result in results {
        let state_icon = result.state_icon(true);
        let sentence = result.sentence();

        let message = format!("{} {}", state_icon, sentence);
        let form = [
            ("message", &message),
            ("user", &opts.pushover_user),
            ("token", &opts.pushover_token),
            (
                "title",
                &format!("HTTP Certificate Check - {}", result.domain_name),
            ),
        ];
        futs.push(pushover_client.post(PUSHOVER_API).form(&form).send());
    }

    futures::future::try_join_all(futs).await?;

    Ok(())
}
