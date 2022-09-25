use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr as _;

use mime::Mime;
use thiserror::Error;
use url::Url;

/// Attachment error.
#[derive(Error, Debug)]
pub enum AttachmentError {
    /// Error from [`std::io`].
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    /// Error from [`ureq`] crate.
    #[error("ureq error: {0}")]
    UReq(#[from] Box<ureq::Error>),
    /// Error from [`url`] crate.
    #[error("attachment URL error: {0}")]
    Url(#[from] url::ParseError),
    /// Failed to infer MIME type, no extra information.
    #[error("unknown MIME type")]
    Infer,
}

/// Notification attachment. Image in most cases.
#[derive(Debug)]
pub struct Attachment<'a> {
    /// Filename.
    pub(crate) filename: Cow<'a, str>,
    /// MIME type, inferred when attached from URL.
    pub(crate) mime: Mime,
    /// Attachment content.
    pub(crate) content: Vec<u8>,
}

impl<'a> Attachment<'a> {
    /// Creates an [`Attachment`].
    pub fn new<T>(filename: T, mime: Mime, content: &[u8]) -> Attachment<'a>
    where
        T: 'a + Into<Cow<'a, str>>,
    {
        Self {
            filename: filename.into(),
            mime,
            content: content.to_vec(),
        }
    }

    /// Creates an [`Attachment`] from path.
    pub async fn from_path<T>(path: T) -> Result<Attachment<'a>, AttachmentError>
    where
        T: AsRef<Path>,
    {
        let mut buffer = Vec::new();
        let mut handle = File::open(path.as_ref())?;
        handle.read_to_end(&mut buffer)?;
        let filename = path
            .as_ref()
            .file_name()
            .map_or("filename", |t| t.to_str().map_or("filename", |t| t));
        let inferred = infer::get(&buffer).ok_or(AttachmentError::Infer)?;
        let mime = Mime::from_str(inferred.mime_type()).map_err(|_e| AttachmentError::Infer)?;
        Ok(Self::new(filename.to_owned(), mime, &buffer))
    }

    /// Creates an [`Attachment`] from URL.
    pub async fn from_url<T>(url: T) -> Result<Attachment<'a>, AttachmentError>
    where
        T: AsRef<str>,
    {
        let parsed = Url::parse(url.as_ref())?;
        let filename = parsed
            .path_segments()
            .map_or("filename", |s| s.last().map_or("filename", |s| s));
        let res = ureq::get(parsed.as_str())
            .call()
            .map_err(|e| AttachmentError::UReq(Box::new(e)))?;
        let mut buffer = Vec::new();
        res.into_reader().read_to_end(&mut buffer)?;
        let inferred = infer::get(&buffer).ok_or(AttachmentError::Infer)?;
        let mime = Mime::from_str(inferred.mime_type()).map_err(|_e| AttachmentError::Infer)?;
        Ok(Self::new(filename.to_owned(), mime, &buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::server_url;

    use mockito::mock;

    #[test]
    fn t_attachment_new() {
        Attachment::new("filename", Mime::from_str("plain/text").unwrap(), &[]);
    }

    #[tokio::test]
    async fn t_from_url() -> Result<(), AttachmentError> {
        let body = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let _m = mock("GET", "/image.png")
            .with_status(200)
            .with_body(body)
            .create();

        let host = server_url();
        let url = format!("{host}/image.png");

        // accepts &str
        let attachment = Attachment::from_url(&url).await?;
        assert_eq!(body.len(), attachment.content.len());

        // accepts Url
        let url = Url::parse(&url)?;
        let attachment = Attachment::from_url(&url).await?;
        assert_eq!(body.len(), attachment.content.len());

        Ok(())
    }

    #[tokio::test]
    async fn t_attach_url() -> Result<(), AttachmentError> {
        let body = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let _m = mock("GET", "/filename.png")
            .with_status(200)
            .with_body(&body)
            .create();

        let host = server_url();
        let u = format!("{host}/filename.png");
        let a = Attachment::from_url(u).await?;
        assert_eq!("filename.png", a.filename);
        assert_eq!("image/png", a.mime.to_string());
        assert_eq!(body.len(), a.content.len());
        Ok(())
    }
}
