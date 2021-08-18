#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

use anyhow::bail;
use atty::Stream;
use structopt::StructOpt;

use pop::notification::{Attachment, Notification};

#[derive(Debug, StructOpt)]
#[structopt(about, author)]
struct Opts {
    /// Verbose mode
    #[structopt(short, long)]
    verbose: bool,
    /// Pushover API token, get it on https://pushover.net/apps/build
    #[structopt(short, long, env = "PUSHOVER_TOKEN")]
    token: String,
    /// Pushover user key, get it on https://pushover.net/
    #[structopt(short, long, env = "PUSHOVER_USER")]
    user: String,
    /// Message to send
    #[structopt(short, long)]
    message: String,
    /// Attachment to send, which is an image usually
    #[structopt(short, long, parse(from_os_str))]
    attachment: Option<PathBuf>,
    /// Download and attach in request body
    #[structopt(short, long)]
    image_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::from_args();

    if opts.attachment.is_some() && opts.image_url.is_some() {
        bail!("either attachment or image_url is given, not both");
    }

    let notification = Notification::new(&opts.token, &opts.user, &opts.message);

    let notification = if let Some(ref image_url) = opts.image_url {
        notification.attach_url(image_url).await?
    } else {
        match parse_attachment(&opts)? {
            Some((filename, mime_type, content)) => {
                let attachment = Attachment::new(filename, mime_type, content);
                notification.attach(attachment)
            }
            None => notification,
        }
    };

    let response = notification.send().await?;
    if opts.verbose {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}

fn read_from_stdin_or_file(opts: &Opts) -> anyhow::Result<Option<Box<dyn BufRead>>> {
    if atty::isnt(Stream::Stdin) {
        // read from STDIN
        Ok(Some(Box::new(BufReader::new(io::stdin()))))
    } else if let Some(ref a) = opts.attachment {
        // read from designated file
        Ok(Some(Box::new(BufReader::new(fs::File::open(a)?))))
    } else {
        // Nothing
        Ok(None)
    }
}

fn parse_attachment(opts: &Opts) -> anyhow::Result<Option<(String, String, Vec<u8>)>> {
    if let Some(mut r) = read_from_stdin_or_file(opts)? {
        let mut content = Vec::new();
        r.read_to_end(&mut content)?;

        let mime_type = match infer::get(&content) {
            Some(m) => m,
            None => bail!("MIME type of attachment is unknown"),
        };

        let filename = match opts.attachment {
            Some(ref f) => match f.to_str() {
                Some(s) => s.to_string(),
                None => bail!("failed to extract filename from attachment"),
            },
            None => format!("file.{}", mime_type.extension()),
        };

        let mime_type = mime_type.to_string();
        Ok(Some((filename, mime_type, content)))
    } else {
        Ok(None)
    }
}
