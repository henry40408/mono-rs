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

//! Pushover is Pushover API wrapper with attachment support in Rust 2021 edition.

use maplit::{hashmap, hashset};
use multipart::client::lazy::Multipart;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Display;
use std::io::Cursor;
use thiserror::Error;

pub use attachment::{Attachment, AttachmentError};

mod attachment;

/// Notification error.
#[derive(Error, Debug)]
pub enum NotificationError {
    /// Error from [`ureq`] crate.
    #[error("ureq error: {0}")]
    UReq(#[from] Box<ureq::Error>),
    /// Error from [`serde_json`] crate.
    #[error("deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
    /// Wrapped [`crate::AttachmentError`].
    #[error("attachment error: {0}")]
    Attachment(#[from] AttachmentError),
    /// HTML and monospace are mutually exclusive. <https://pushover.net/api#html>
    #[error("html and monospace are mutually exclusive")]
    HTMLMonospace,
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Pushover API parameters <https://pushover.net/api#messages> and attachment.
#[derive(Default, Debug)]
pub struct Notification<'a> {
    token: Cow<'a, str>,
    identifier: Cow<'a, str>,
    message: Cow<'a, str>,
    /// Your user's device name to send the message directly to that device,
    /// rather than all of the user's devices (multiple devices may be separated by a comma).
    /// <https://pushover.net/api#identifiers>
    pub device: Option<&'a str>,
    /// Your message's title, otherwise your app's name is used. <https://pushover.net/api#messages>
    pub title: Option<&'a str>,
    /// To enable HTML formatting. <https://pushover.net/api#html>
    pub html: Option<HTML>,
    /// To enable monospace messages. <https://pushover.net/api#html>
    pub monospace: Option<Monospace>,
    /// Messages are stored on the Pushover servers with a timestamp of
    /// when they were initially received through the API. <https://pushover.net/api#html>
    pub timestamp: Option<u64>,
    /// Messages may be sent with a different priority that affects
    /// how the message is presented to the user. <https://pushover.net/api#priority>
    pub priority: Option<Priority>,
    /// A supplementary URL to show with your message. <https://pushover.net/api#urls>
    pub url: Option<&'a str>,
    /// A title for your supplementary URL,
    /// otherwise just the URL is shown. <https://pushover.net/api#urls>
    pub url_title: Option<&'a str>,
    /// Users can choose from a number of different default sounds
    /// to play when receiving notifications. <https://pushover.net/api#sounds>
    pub sound: Option<Sound>,
    /// Optional [`Attachment`].
    pub attachment: Option<&'a Attachment<'a>>,
}

/// To enable HTML formatting. <https://pushover.net/api#html>
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::Display, strum::EnumString)]
pub enum HTML {
    /// Plain text (default)
    #[strum(to_string = "0", serialize = "plain")]
    Plain,
    /// HTML
    #[strum(to_string = "1", serialize = "html")]
    HTML,
}

/// To enable monospace messages. <https://pushover.net/api#html>
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::Display, strum::EnumString)]
pub enum Monospace {
    /// Normal (default)
    #[strum(to_string = "0", serialize = "normal")]
    Normal,
    /// Monospace
    #[strum(to_string = "1", serialize = "monospace")]
    Monospace,
}

/// Messages may be sent with a different priority
/// that affects how the message is presented to the user. <https://pushover.net/api#priority>
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::Display, strum::EnumString)]
pub enum Priority {
    /// Normal (default)
    #[strum(to_string = "0", serialize = "normal")]
    Normal,
    /// Lowest
    #[strum(to_string = "-2", serialize = "lowest")]
    Lowest,
    /// Low
    #[strum(to_string = "-1", serialize = "low")]
    Low,
    /// High
    #[strum(to_string = "1", serialize = "high")]
    High,
    /// Emergency
    #[strum(to_string = "2", serialize = "emergency")]
    Emergency,
}

/// Users can choose from a number of different default sounds
/// to play when receiving notifications. <https://pushover.net/api#sounds>
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::Display, strum::EnumString)]
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

#[cfg(test)]
fn server_url() -> String {
    mockito::server_url()
}

#[cfg(not(test))]
fn server_url() -> String {
    "https://api.pushover.net".to_string()
}

/// Shorthand function to send notification to Pushover.
/// ```
/// use pushover::send_notification;
/// send_notification("token", "user", "message");
/// send_notification("token", "group", "message");
/// ```
pub async fn send_notification<'a, S>(
    token: S,
    identifier: S,
    message: S,
) -> Result<Response, NotificationError>
where
    S: Into<Cow<'a, str>>,
{
    Notification::new(token, identifier, message).send().await
}

#[doc(hidden)]
pub fn sanitize_message<'a, T>(message: T) -> Cow<'a, str>
where
    T: Into<Cow<'a, str>>,
{
    let tags = hashset!["b", "i", "u", "font", "a"];
    let tag_attrs = hashmap![
        "a" => hashset!["href"],
        "font" => hashset!["color"],
    ];
    let message = message.into();
    // Builder consumes tags and tag_attrs unless maintainer changes method signatures
    ammonia::Builder::default()
        .tags(tags)
        .tag_attributes(tag_attrs)
        .clean(message.as_ref())
        .to_string()
        .into()
}

fn add_optional_text<T: Display>(f: &mut Multipart, n: &'static str, v: Option<T>) {
    if let Some(v) = v {
        f.add_text(n, v.to_string());
    }
}

impl<'a> Notification<'a> {
    /// Creates a [`Notification`].
    ///
    /// Once you have an API token, you'll need the user key and optional device name
    /// for each user to which you are pushing notifications. Instead of a user key,
    /// a group key may be supplied. Group keys look identical to user keys and from
    /// your application's perspective, you do not need to distinguish between them.
    ///
    /// ```rust
    /// # use pushover::Notification;
    /// // Notify user
    /// Notification::new("token", "user", "message");
    /// // Notify group of users
    /// Notification::new("token", "group", "message");
    /// ```
    pub fn new<T>(token: T, identifier: T, message: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        Self {
            token: token.into(),
            identifier: identifier.into(),
            message: message.into(),
            ..Default::default()
        }
    }

    /// Send [`Notification`] to Pushover.
    pub async fn send(&self) -> Result<Response, NotificationError> {
        // HTML and monospace are mutually exclusive <https://pushover.net/api#html>
        if self.html == Some(HTML::HTML) && self.monospace == Some(Monospace::Monospace) {
            return Err(NotificationError::HTMLMonospace);
        }

        let mut form = Multipart::new();

        form.add_text("token", self.token.to_string());
        form.add_text("user", self.identifier.to_string()); // User or group key
        form.add_text("message", sanitize_message(self.message.clone()));

        add_optional_text(&mut form, "device", self.device.as_ref());
        add_optional_text(&mut form, "title", self.title.as_ref());
        add_optional_text(&mut form, "html", self.html.as_ref());
        add_optional_text(&mut form, "monospace", self.monospace.as_ref());
        add_optional_text(&mut form, "timestamp", self.timestamp.as_ref());
        add_optional_text(&mut form, "priority", self.priority.as_ref());
        add_optional_text(&mut form, "url", self.url.as_ref());
        add_optional_text(&mut form, "url_title", self.url_title.as_ref());
        add_optional_text(&mut form, "sound", self.sound.as_ref());

        if let Some(a) = self.attachment {
            let reader = Cursor::new(&a.content);
            form.add_stream(
                "attachment",
                reader,
                Some(a.filename.clone()),
                Some(a.mime.clone()),
            );
        }

        let host = server_url();
        let uri = format!("{host}/1/messages.json");

        let form = form.prepare().map_err(|e| e.error)?;
        let boundary = form.boundary();
        let content_type = format!("multipart/form-data; boundary={boundary}");
        let response = ureq::post(&uri)
            .set("Content-Type", &content_type)
            .send(form)
            .map_err(|e| NotificationError::UReq(Box::new(e)))?;

        let body = response.into_string().map_err(NotificationError::Io)?;
        serde_json::from_str(&body).map_err(NotificationError::Deserialize)
    }
}

/// Pushover API response. <https://pushover.net/api#response>
#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    /// If POST request to API was valid, we will receive an HTTP 200 (OK) status, with a JSON object containing a status code of `1`.
    pub status: u8,
    /// The `request` parameter returned from all API calls is a randomly-generated unique token that we have associated with your request.
    pub request: String,
    /// ...and an `errors` array detailing which parameters were invalid.
    pub errors: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr as _;

    use mime::Mime;
    use mockito::mock;

    #[test]
    fn t_new() {
        build_notification();
    }

    #[tokio::test]
    async fn t_send() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"00000000-0000-0000-0000-000000000000"}"#)
            .create();

        let n = build_notification();

        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("00000000-0000-0000-0000-000000000000", res.request);
        assert!(res.errors.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn t_device() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"00000000-0000-0000-0000-000000000000"}"#)
            .create();

        let mut n = build_notification();
        n.device = Some("device".into());

        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("00000000-0000-0000-0000-000000000000", res.request);
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
    fn t_html() -> Result<(), strum::ParseError> {
        assert_eq!("0", HTML::Plain.to_string());
        assert_eq!(HTML::Plain, HTML::from_str("0")?);
        assert_eq!(HTML::Plain, HTML::from_str("plain")?);
        assert_eq!("1", HTML::HTML.to_string());
        assert_eq!(HTML::HTML, HTML::from_str("1")?);
        assert_eq!(HTML::HTML, HTML::from_str("html")?);
        Ok(())
    }

    #[test]
    fn t_monospace() -> Result<(), strum::ParseError> {
        assert_eq!("0", Monospace::Normal.to_string());
        assert_eq!(Monospace::Normal, Monospace::from_str("0")?);
        assert_eq!(Monospace::Normal, Monospace::from_str("normal")?);
        assert_eq!("1", Monospace::Monospace.to_string());
        assert_eq!(Monospace::Monospace, Monospace::from_str("1")?);
        assert_eq!(Monospace::Monospace, Monospace::from_str("monospace")?);
        Ok(())
    }

    #[test]
    fn t_priority() -> Result<(), strum::ParseError> {
        assert_eq!("-2", Priority::Lowest.to_string());
        assert_eq!(Priority::Lowest, Priority::from_str("-2")?);
        assert_eq!(Priority::Lowest, Priority::from_str("lowest")?);
        assert_eq!("-1", Priority::Low.to_string());
        assert_eq!(Priority::Low, Priority::from_str("-1")?);
        assert_eq!(Priority::Low, Priority::from_str("low")?);
        assert_eq!("0", Priority::Normal.to_string());
        assert_eq!(Priority::Normal, Priority::from_str("0")?);
        assert_eq!(Priority::Normal, Priority::from_str("normal")?);
        assert_eq!("1", Priority::High.to_string());
        assert_eq!(Priority::High, Priority::from_str("1")?);
        assert_eq!(Priority::High, Priority::from_str("high")?);
        assert_eq!("2", Priority::Emergency.to_string());
        assert_eq!(Priority::Emergency, Priority::from_str("2")?);
        assert_eq!(Priority::Emergency, Priority::from_str("emergency")?);
        Ok(())
    }

    #[test]
    fn t_sound() -> Result<(), strum::ParseError> {
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
    async fn t_attach_and_send() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"00000000-0000-0000-0000-000000000000"}"#)
            .create();

        let mut n = build_notification();
        let a = Attachment::new("filename", Mime::from_str("plain/text").unwrap(), &[]);
        n.attachment = Some(&a);

        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("00000000-0000-0000-0000-000000000000", res.request);
        Ok(())
    }

    #[tokio::test]
    async fn t_attach_url_and_send() -> Result<(), NotificationError> {
        let body = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"00000000-0000-0000-0000-000000000000"}"#)
            .create();

        let _n = mock("GET", "/filename.png")
            .with_status(200)
            .with_body(body)
            .create();

        let host = server_url();
        let u = format!("{host}/filename.png");

        let a = Attachment::from_url(&u).await?;
        assert_eq!("filename.png", a.filename);
        assert_eq!("image/png", a.mime.to_string());
        assert_eq!(body.len(), a.content.len());

        let mut n = build_notification();
        n.attachment = Some(&a);

        let res = n.send().await?;
        assert_eq!(1, res.status);
        assert_eq!("00000000-0000-0000-0000-000000000000", res.request);
        Ok(())
    }

    #[test]
    fn t_sanitized_message() {
        let s = "<b>bold</b>";
        assert_eq!(s, sanitize_message(s));

        let s = "<i>italic</i>";
        assert_eq!(s, sanitize_message(s));

        let s = "<u>underline</u>";
        assert_eq!(s, sanitize_message(s));

        let s = "<font color=\"#000000\">font</font>";
        assert_eq!(s, sanitize_message(s));

        let s = "<a href=\"https://badssl.com/\">link</a>";
        assert_eq!(
            "<a href=\"https://badssl.com/\" rel=\"noopener noreferrer\">link</a>",
            sanitize_message(s)
        );

        let s = "<script>alert('XSS');</script>";
        assert_eq!("", sanitize_message(s));
    }

    #[tokio::test]
    async fn t_sned_message() -> Result<(), NotificationError> {
        let _m = mock("POST", "/1/messages.json")
            .with_status(200)
            .with_body(r#"{"status":1,"request":"00000000-0000-0000-0000-000000000000"}"#)
            .create();

        let res = send_notification("token", "user", "message").await?;
        assert_eq!(1, res.status);
        assert_eq!("00000000-0000-0000-0000-000000000000", res.request);
        assert!(res.errors.is_none());
        Ok(())
    }
}
