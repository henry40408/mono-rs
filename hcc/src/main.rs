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

use clap::Parser;

use hcc::Checker;

#[derive(Debug, Default, Parser)]
#[clap(author, about, version)]
struct Opts {
    /// ASCII
    #[clap(short, long)]
    ascii: bool,
    /// Verbose mode
    #[clap(short, long)]
    verbose: bool,
    #[clap(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Parser)]
enum Command {
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
}

#[tokio::main]
async fn main() {
    let opts: Opts = Opts::parse();
    if let Some(Command::Check {
        ref domain_names,
        grace_in_days,
    }) = opts.command
    {
        let domain_names: Vec<&str> = domain_names.iter().map(AsRef::as_ref).collect();
        check_command(&opts, &domain_names, grace_in_days).await;
    }
}

async fn check_command<T>(opts: &Opts, domain_names: &[T], grace_in_days: i64)
where
    T: AsRef<str>,
{
    let mut client = Checker::default();
    client.ascii = opts.ascii;
    client.elapsed = opts.verbose;
    client.grace_in_days = grace_in_days;

    let results = client.check_many(domain_names).await;
    for result in results.iter() {
        println!("{}", result);
    }
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
        check_command(&opts, &["sha256.badssl.com"], 7).await;
    }

    #[tokio::test]
    async fn t_check_command_expired() {
        let opts = build_opts();
        check_command(&opts, &["expired.badssl.com"], 7).await;
    }
}
