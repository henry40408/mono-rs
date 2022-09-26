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

//! po is a command line application based on Pushover API.
//!
//! If Pushover API token / key is "token" and user key is "user",
//!
//! ```
//! $ po -t token -u user -m message
//! ```
//!
//! Or you can set environment variables instead,
//!
//! ```
//! $ export PUSHOVER_TOKEN=token
//! $ export PUSHOVER_USER=user
//! $ po -m message
//! ```
//!
//! For more information,
//!
//! ```
//! $ po -h
//! ```

use anyhow::bail;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use log::{debug, Level};
use logging_timer::{finish, stimer};

use pushover::{Attachment, Monospace, Notification, Priority, Sound, HTML};

#[doc(hidden)]
#[derive(Parser)]
#[clap(about, author, version)]
struct Opts {
    /// Your application's API token. <https://pushover.net/api#identifiers>
    #[clap(short, long, env = "PUSHOVER_TOKEN")]
    token: String,
    /// The user / group key (not e-mail address) of your user (or you). <https://pushover.net/api#identifiers>
    #[clap(short, long, env = "PUSHOVER_USER")]
    user: String,
    /// Your message. <https://pushover.net/api#messages>
    #[clap(short, long)]
    message: String,
    /// Verbose.
    #[clap(short, long)]
    verbose: bool,
    /// To enable HTML formatting. monospace may not be used if html is used, and vice versa. <https://pushover.net/api#html>
    #[clap(long)]
    html: bool,
    /// To enable monospace messages. monospace may not be used if html is used, and vice versa. <https://pushover.net/api#html>
    #[clap(long)]
    monospace: bool,
    /// Your user's device name to send the message directly to that device, rather than all of the user's devices. <https://pushover.net/api#identifiers>
    #[clap(long)]
    device: Option<String>,
    /// Your message's title, otherwise your app's name is used. <https://pushover.net/api#messages>
    #[clap(long)]
    title: Option<String>,
    /// A Unix timestamp of your message's date and time to display to the user, rather than the time your message is received by our API. <https://pushover.net/api#timestamp>
    #[clap(long)]
    timestamp: Option<u64>,
    /// Attach file as notification attachment.
    #[clap(short, long)]
    file: Option<PathBuf>,
    /// Messages may be sent with a different priority that affects how the message is presented to the user
    /// e.g. -2, -1, 0, 1, 2, lowest, low, normal, high, emergency. <https://pushover.net/api#priority>
    #[clap(long, allow_hyphen_values = true)]
    priority: Option<String>,
    /// Users can choose from a number of different default sounds to play when receiving notifications. <https://pushover.net/api#sounds>
    #[clap(long)]
    sound: Option<String>,
    /// A supplementary URL to show with your message. <https://pushover.net/api#urls>
    #[clap(long)]
    url: Option<String>,
    /// A title for your supplementary URL, otherwise just the URL is shown. <https://pushover.net/api#urls>
    #[clap(long)]
    url_title: Option<String>,
}

#[doc(hidden)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use std::io::Read as _;

    pretty_env_logger::init();

    let opts: Opts = Opts::parse();

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
        debug!("load attachment from {p:?}");
        Some(Attachment::from_path(p).await?)
    } else if atty::isnt(atty::Stream::Stdin) {
        debug!("load attachment from standard input");
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf)?;
        Some(Attachment::try_from(buf)?)
    } else {
        None
    };
    notification.attachment = attachment.as_ref();

    let tmr = stimer!(Level::Debug; "NOTIFY");
    let res = notification.send().await?;
    finish!(tmr);

    if res.status != 1 {
        bail!(format!("{res:?}"));
    } else if opts.verbose {
        println!("{res:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::Opts;

    #[test]
    fn test_negative_priority() {
        let parsed: Opts = Opts::from_iter(vec![
            "--",
            "-t",
            "token",
            "-u",
            "user",
            "-m",
            "message",
            "--priority",
            "-2",
        ]);
        assert_eq!(parsed.priority, Some("-2".to_string()));

        let parsed: Opts = Opts::from_iter(vec![
            "--",
            "-t",
            "token",
            "-u",
            "user",
            "-m",
            "message",
            "--priority",
            "-1",
        ]);
        assert_eq!(parsed.priority, Some("-1".to_string()));
    }
}
