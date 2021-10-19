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

//! Pushover is Pushover API wrapper with attachment support in Rust 2018 edition

use maplit::{hashmap, hashset};
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod attachment;

pub use attachment::{Attachment, AttachmentError};

/// Pushover API request <https://pushover.net/api#messages>
#[derive(Default, Debug)]
pub struct Request<'a> {
    token: &'a str,
    user: &'a str,
    message: &'a str,
    /// your user's device name to send the message directly to that device, rather than all of the user's devices (multiple devices may be separated by a comma) <https://pushover.net/api#identifiers>
    pub device: Option<&'a str>,
    /// your message's title, otherwise your app's name is used <https://pushover.net/api#messages>
    pub title: Option<&'a str>,
    /// To enable HTML formatting <https://pushover.net/api#html>
    pub html: Option<HTML>,
    /// To enable monospace messages <https://pushover.net/api#html>
    pub monospace: Option<Monospace>,
    /// Messages are stored on the Pushover servers with a timestamp of when they were initially received through the API <https://pushover.net/api#html>
    pub timestamp: Option<u64>,
    /// Messages may be sent with a different priority that affects how the message is presented to the user <https://pushover.net/api#priority>
    pub priority: Option<Priority>,
    /// a supplementary URL to show with your message <https://pushover.net/api#urls>
    pub url: Option<&'a str>,
    /// a title for your supplementary URL, otherwise just the URL is shown <https://pushover.net/api#urls>
    pub url_title: Option<&'a str>,
    /// Users can choose from a number of different default sounds to play when receiving notifications <https://pushover.net/api#sounds>
    pub sound: Option<Sound>,
}

/// To enable HTML formatting <https://pushover.net/api#html>
#[derive(Clone, Copy, Debug, PartialEq, strum::ToString, strum::EnumString)]
pub enum HTML {
    /// Plain text
    #[strum(serialize = "0")]
    None,
    /// HTML
    #[strum(serialize = "1")]
    Enabled,
}

/// To enable monospace messages <https://pushover.net/api#html>
#[derive(Clone, Copy, Debug, PartialEq, strum::ToString, strum::EnumString)]
pub enum Monospace {
    /// Normal
    #[strum(serialize = "0")]
    None,
    /// Monospace
    #[strum(serialize = "1")]
    Enabled,
}

/// Messages may be sent with a different priority that affects how the message is presented to the user <https://pushover.net/api#priority>
#[derive(Clone, Copy, Debug, PartialEq, strum::ToString, strum::EnumString)]
pub enum Priority {
    /// Normal
    #[strum(serialize = "0")]
    Normal,
    /// Lowest
    #[strum(serialize = "-2")]
    Lowest,
    /// Low
    #[strum(serialize = "-1")]
    Low,
    /// High
    #[strum(serialize = "1")]
    High,
    /// Emergency
    #[strum(serialize = "2")]
    Emergency,
}

/// Users can choose from a number of different default sounds to play when receiving notifications <https://pushover.net/api#sounds>
#[derive(Clone, Copy, Debug, PartialEq, strum::ToString, strum::EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Sound {
    /// pushover - Pushover (default)
    Pushover,
    /// bike - Bike
    Bike,
    /// bugle - Bugle
    Bugle,
    /// cashregister - Cash Register
    CashRegister,
    /// classical - Classical
    Classical,
    /// cosmic - Cosmic
    Cosmic,
    /// falling - Falling
    Falling,
    /// gamelan - Gamelan
    GameLan,
    /// incoming - Incoming
    Incoming,
    /// intermission - Intermission
    Intermission,
    /// magic - Magic
    Magic,
    /// mechanical - Mechanical
    Mechanical,
    /// pianobar - Piano Bar
    PianoBar,
    /// siren - Siren
    Siren,
    /// spacealarm - Space Alarm
    SpaceAlarm,
    /// tugboat - Tug Boat
    Tugboat,
    /// alien - Alien Alarm (long)
    Alien,
    /// climb - Climb (long)
    Climb,
    /// persistent - Persistent (long)
    Persistent,
    /// echo - Pushover Echo (long)
    Echo,
    /// updown - Up Down (long)
    UpDown,
    /// vibrate - Vibrate Only
    Vibrate,
    /// none - None (silent)
    None,
}

/// Notification error
#[derive(Error, Debug)]
pub enum NotificationError {
    /// Error from [`reqwest`] crate
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// Error from [`serde_json`] crate
    #[error("deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
    /// Wrapped [`crate::AttachmentError`]
    #[error("attachment error: {0}")]
    Attachment(#[from] AttachmentError),
}

/// Request wrapped with attachment
#[derive(Default, Debug)]
pub struct Notification<'a> {
    /// Actual request sent to Pushover API
    pub request: Request<'a>,
    attachment: Option<&'a Attachment<'a>>,
}

#[cfg(test)]
fn server_url() -> String {
    mockito::server_url()
}

#[cfg(not(test))]
fn server_url() -> String {
    "https://api.pushover.net".to_string()
}

impl<'a> Notification<'a> {
    /// Creates a [`Notification`]
    pub fn new(token: &'a str, user: &'a str, message: &'a str) -> Self {
        Self {
            request: Request {
                token,
                user,
                message,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Attach an [`Attachment`]
    pub fn attach(&mut self, attachment: &'a Attachment) {
        self.attachment = Some(attachment);
    }

    /// Sanitize message in [`Request`]
    pub fn sanitized_message(&'a self) -> String {
        let tags = hashset!["b", "i", "u", "font", "a"];
        let tag_attrs = hashmap![
            "a"=>hashset!["href"],
            "font"=>hashset!["color"],
        ];
        ammonia::Builder::default()
            .tags(tags)
            .tag_attributes(tag_attrs)
            .clean(self.request.message)
            .to_string()
    }

    /// Send [`Request`] to Pushover API
    pub async fn send(&'a self) -> Result<Response, NotificationError> {
        let form = multipart::Form::new()
            .text("token", self.request.token.to_string())
            .text("user", self.request.user.to_string())
            .text("message", self.sanitized_message());

        let form = Self::append_part(form, "device", self.request.device.as_ref());
        let form = Self::append_part(form, "title", self.request.title.as_ref());
        let form = Self::append_part(form, "html", self.request.html.as_ref());
        let form = Self::append_part(form, "monospace", self.request.monospace.as_ref());
        let form = Self::append_part(form, "timestamp", self.request.timestamp.as_ref());
        let form = Self::append_part(form, "priority", self.request.priority.as_ref());
        let form = Self::append_part(form, "url", self.request.url.as_ref());
        let form = Self::append_part(form, "url_title", self.request.url_title.as_ref());
        let form = Self::append_part(form, "sound", self.request.sound.as_ref());

        let form = if let Some(a) = self.attachment {
            let part = multipart::Part::bytes(a.content.clone())
                .file_name(a.filename.to_string())
                .mime_str(a.mime_type)?;
            form.part("attachment", part)
        } else {
            form
        };

        let uri = format!("{0}/1/messages.json", server_url());
        let client = reqwest::Client::new();
        let body = client
            .post(&uri)
            .multipart(form)
            .send()
            .await?
            .text()
            .await?;
        match serde_json::from_str(&body) {
            Ok(r) => Ok(r),
            Err(e) => Err(NotificationError::Deserialize(e)),
        }
    }

    fn append_part<T: ToString>(
        form: multipart::Form,
        name: &'static str,
        value: Option<&T>,
    ) -> multipart::Form {
        if let Some(v) = value {
            form.text(name, v.to_string())
        } else {
            form
        }
    }
}

/// Pushover API response <https://pushover.net/api#response>
#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    /// If POST request to API was valid, we will receive an HTTP 200 (OK) status, with a JSON object containing a status code of `1`.
    pub status: u8,
    /// The `request` parameter returned from all API calls is a randomly-generated unique token that we have associated with your request.
    pub request: String,
    /// …and an `errors` array detailing which parameters were invalid
    pub errors: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use mockito::mock;
    use std::str::FromStr;

    use crate::attachment::Attachment;
    use crate::{server_url, Monospace, Notification, NotificationError, Priority, Sound, HTML};

    #[test]
    fn test_new() {
        build_notification();
    }

    #[tokio::test]
    async fn test_send() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"647d2300-702c-4b38-8b2f-d56326ae460b"}"#)
            .create();
        let n = build_notification();
        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("647d2300-702c-4b38-8b2f-d56326ae460b", res.request);
        assert!(res.errors.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_device() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"647d2300-702c-4b38-8b2f-d56326ae460b"}"#)
            .create();

        let mut n = build_notification();
        n.request.device = Some("device".into());

        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("647d2300-702c-4b38-8b2f-d56326ae460b", res.request);
        assert!(res.errors.is_none());

        Ok(())
    }

    fn build_notification<'a>() -> Notification<'a> {
        let user = "user";
        let token = "token";
        let message = "message";
        Notification::new(token, user, message)
    }

    #[test]
    fn test_html() -> Result<(), strum::ParseError> {
        assert_eq!("0", HTML::None.to_string());
        assert_eq!(HTML::None, HTML::from_str("0")?);
        assert_eq!("1", HTML::Enabled.to_string());
        assert_eq!(HTML::Enabled, HTML::from_str("1")?);
        Ok(())
    }

    #[test]
    fn test_monospace() -> Result<(), strum::ParseError> {
        assert_eq!("0", Monospace::None.to_string());
        assert_eq!(Monospace::None, Monospace::from_str("0")?);
        assert_eq!("1", Monospace::Enabled.to_string());
        assert_eq!(Monospace::Enabled, Monospace::from_str("1")?);
        Ok(())
    }

    #[test]
    fn test_priority() -> Result<(), strum::ParseError> {
        assert_eq!("-2", Priority::Lowest.to_string());
        assert_eq!(Priority::Lowest, Priority::from_str("-2")?);
        assert_eq!("-1", Priority::Low.to_string());
        assert_eq!(Priority::Low, Priority::from_str("-1")?);
        assert_eq!("0", Priority::Normal.to_string());
        assert_eq!(Priority::Normal, Priority::from_str("0")?);
        assert_eq!("1", Priority::High.to_string());
        assert_eq!(Priority::High, Priority::from_str("1")?);
        assert_eq!("2", Priority::Emergency.to_string());
        assert_eq!(Priority::Emergency, Priority::from_str("2")?);
        Ok(())
    }

    #[test]
    fn test_sound() -> Result<(), strum::ParseError> {
        assert_eq!("pushover", Sound::Pushover.to_string());
        assert_eq!(Sound::Pushover, Sound::from_str("pushover")?);
        assert_eq!("bike", Sound::Bike.to_string());
        assert_eq!(Sound::Bike, Sound::from_str("bike")?);
        assert_eq!("bugle", Sound::Bugle.to_string());
        assert_eq!(Sound::Bugle, Sound::from_str("bugle")?);
        assert_eq!("cashregister", Sound::CashRegister.to_string());
        assert_eq!(Sound::CashRegister, Sound::from_str("cashregister")?);
        assert_eq!("classical", Sound::Classical.to_string());
        assert_eq!(Sound::Classical, Sound::from_str("classical")?);
        assert_eq!("cosmic", Sound::Cosmic.to_string());
        assert_eq!(Sound::Cosmic, Sound::from_str("cosmic")?);
        assert_eq!("falling", Sound::Falling.to_string());
        assert_eq!(Sound::Falling, Sound::from_str("falling")?);
        assert_eq!("gamelan", Sound::GameLan.to_string());
        assert_eq!(Sound::GameLan, Sound::from_str("gamelan")?);
        assert_eq!("incoming", Sound::Incoming.to_string());
        assert_eq!(Sound::Incoming, Sound::from_str("incoming")?);
        assert_eq!("intermission", Sound::Intermission.to_string());
        assert_eq!(Sound::Intermission, Sound::from_str("intermission")?);
        assert_eq!("magic", Sound::Magic.to_string());
        assert_eq!(Sound::Magic, Sound::from_str("magic")?);
        assert_eq!("mechanical", Sound::Mechanical.to_string());
        assert_eq!(Sound::Mechanical, Sound::from_str("mechanical")?);
        assert_eq!("pianobar", Sound::PianoBar.to_string());
        assert_eq!(Sound::PianoBar, Sound::from_str("pianobar")?);
        assert_eq!("siren", Sound::Siren.to_string());
        assert_eq!(Sound::Siren, Sound::from_str("siren")?);
        assert_eq!("spacealarm", Sound::SpaceAlarm.to_string());
        assert_eq!(Sound::SpaceAlarm, Sound::from_str("spacealarm")?);
        assert_eq!("tugboat", Sound::Tugboat.to_string());
        assert_eq!(Sound::Tugboat, Sound::from_str("tugboat")?);
        assert_eq!("alien", Sound::Alien.to_string());
        assert_eq!(Sound::Alien, Sound::from_str("alien")?);
        assert_eq!("climb", Sound::Climb.to_string());
        assert_eq!(Sound::Climb, Sound::from_str("climb")?);
        assert_eq!("persistent", Sound::Persistent.to_string());
        assert_eq!(Sound::Persistent, Sound::from_str("persistent")?);
        assert_eq!("echo", Sound::Echo.to_string());
        assert_eq!(Sound::Echo, Sound::from_str("echo")?);
        assert_eq!("updown", Sound::UpDown.to_string());
        assert_eq!(Sound::UpDown, Sound::from_str("updown")?);
        assert_eq!("vibrate", Sound::Vibrate.to_string());
        assert_eq!(Sound::Vibrate, Sound::from_str("vibrate")?);
        assert_eq!("none", Sound::None.to_string());
        assert_eq!(Sound::None, Sound::from_str("none")?);
        Ok(())
    }

    #[tokio::test]
    async fn test_attach_and_send() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"647d2300-702c-4b38-8b2f-d56326ae460b"}"#)
            .create();

        let mut n = build_notification();
        let a = Attachment::new("filename", "plain/text", &[]);
        n.attach(&a);

        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("647d2300-702c-4b38-8b2f-d56326ae460b", res.request);
        Ok(())
    }

    #[tokio::test]
    async fn test_attach_url_and_send() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"647d2300-702c-4b38-8b2f-d56326ae460b"}"#)
            .create();

        let _n = mock("GET", "/filename.png")
            .with_status(200)
            .with_body(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
            .create();

        let mut n = build_notification();
        let u = format!("{}/filename.png", server_url());

        let a = Attachment::from_url(&u).await?;
        assert_eq!("filename.png", a.filename);
        assert_eq!("image/png", a.mime_type);
        assert!(a.content.len() > 0);

        n.attach(&a);

        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("647d2300-702c-4b38-8b2f-d56326ae460b", res.request);
        Ok(())
    }

    #[test]
    fn test_sanitized_message() {
        let mut n = build_notification();

        let s = "<b>bold</b>";
        n.request.message = s.into();
        assert_eq!(s, n.sanitized_message());

        let s = "<i>italic</i>";
        n.request.message = s.into();
        assert_eq!(s, n.sanitized_message());

        let s = "<u>underline</u>";
        n.request.message = s.into();
        assert_eq!(s, n.sanitized_message());

        let s = "<font color=\"#000000\">font</font>";
        n.request.message = s.into();
        assert_eq!(s, n.sanitized_message());

        n.request.message = "<a href=\"https://badssl.com/\">link</a>".into();
        assert_eq!(
            "<a href=\"https://badssl.com/\" rel=\"noopener noreferrer\">link</a>",
            n.sanitized_message()
        );

        n.request.message = "<script>alert('XSS');</script>".into();
        assert_eq!("", n.sanitized_message());
    }
}
