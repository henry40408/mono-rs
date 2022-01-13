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

//! po is a command line application based on Pushover API

use std::path::PathBuf;
use std::str::FromStr;

use structopt::StructOpt;

use pushover::{Attachment, Monospace, Notification, Priority, Sound, HTML};

#[derive(StructOpt)]
#[structopt(about, author)]
struct Opts {
    /// your application's API token <https://pushover.net/api#identifiers>
    #[structopt(short, long, env = "PUSHOVER_TOKEN")]
    token: String,
    /// the user / group key (not e-mail address) of your user (or you) <https://pushover.net/api#identifiers>
    #[structopt(short, long, env = "PUSHOVER_USER")]
    user: String,
    /// your message <https://pushover.net/api#messages>
    #[structopt(short, long)]
    message: String,
    /// verbose
    #[structopt(short, long)]
    verbose: bool,
    /// To enable HTML formatting <https://pushover.net/api#html>
    #[structopt(long)]
    html: bool,
    /// To enable monospace messages <https://pushover.net/api#html>
    #[structopt(long)]
    monospace: bool,
    /// your user's device name to send the message directly to that device, rather than all of the user's devices <https://pushover.net/api#identifiers>
    #[structopt(long)]
    device: Option<String>,
    /// your message's title, otherwise your app's name is used <https://pushover.net/api#messages>
    #[structopt(long)]
    title: Option<String>,
    /// a Unix timestamp of your message's date and time to display to the user, rather than the time your message is received by our API <https://pushover.net/api#timestamp>
    #[structopt(long)]
    timestamp: Option<u64>,
    /// attach file as notification attachment
    #[structopt(short, long)]
    file: Option<PathBuf>,
    /// Messages may be sent with a different priority that affects how the message is presented to the user e.g. -2, -1, 0, 1, 2 <https://pushover.net/api#priority>
    #[structopt(long)]
    priority: Option<String>,
    /// Users can choose from a number of different default sounds to play when receiving notifications <https://pushover.net/api#sounds>
    #[structopt(long)]
    sound: Option<String>,
    /// a supplementary URL to show with your message <https://pushover.net/api#urls>
    #[structopt(long)]
    url: Option<String>,
    /// a title for your supplementary URL, otherwise just the URL is shown <https://pushover.net/api#urls>
    #[structopt(long)]
    url_title: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::from_args();

    let mut notification = Notification::new(&opts.token, &opts.user, &opts.message);
    notification.device = opts.device.as_deref();
    notification.title = opts.title.as_deref();
    notification.timestamp = opts.timestamp;
    notification.priority = opts
        .priority
        .as_deref()
        .and_then(|p| Priority::from_str(p).ok());
    notification.sound = opts.sound.as_deref().and_then(|s| Sound::from_str(s).ok());

    notification.url = opts.url.as_deref();
    notification.url_title = opts.url_title.as_deref();

    notification.html = opts.html.then(|| HTML::HTML);
    notification.monospace = opts.monospace.then(|| Monospace::Monospace);

    let attachment = if let Some(ref p) = opts.file {
        Some(Attachment::from_path(p).await?)
    } else {
        None
    };
    notification.attachment = attachment.as_ref();

    // send request
    let res = notification.send().await?;
    if opts.verbose {
        println!("{:?}", res);
    }

    Ok(())
}
