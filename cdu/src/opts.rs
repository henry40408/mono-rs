use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about, author)]
pub struct Opts {
    /// Cloudflare token
    #[structopt(short, long, env = "CLOUDFLARE_TOKEN")]
    pub(crate) token: String,
    /// Cloudflare zone name
    #[structopt(short, long, env = "CLOUDFLARE_ZONE")]
    pub(crate) zone: String,
    /// Cloudflare records separated with comma e.g. a.x.com,b.x.com
    #[structopt(short, long, env = "CLOUDFLARE_RECORDS")]
    records: String,
    /// Debug mode
    #[structopt(long)]
    pub(crate) debug: bool,
    /// Daemon mode
    #[structopt(short, long, env = "DAEMON")]
    pub(crate) daemon: bool,
    /// Cron. Only in effect in daemon mode
    #[structopt(short, long, default_value = "0 */5 * * * * *", env = "CRON")]
    pub(crate) cron: String,
    /// Cache duration in seconds, give 0 to disable
    #[structopt(short = "s", long, default_value = "0", env = "CACHE_SECONDS")]
    pub(crate) cache_seconds: u64,
}

impl Opts {
    pub(crate) fn record_name_list(&self) -> Vec<String> {
        self.records.split(',').map(String::from).collect()
    }
}
