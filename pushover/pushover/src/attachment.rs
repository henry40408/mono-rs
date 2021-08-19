use std::fs::File;
use std::io::Read;
use std::path::Path;
use thiserror::Error;
use url::Url;

/// Attachment error
#[derive(Error, Debug)]
pub enum AttachmentError {
    /// Error from [`std::io`]
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    /// Error from [`reqwest`] crate
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// Error from [`url`] crate
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),
    /// Failed to infer MIME type, no extra information included
    #[error("unknown MIME type")]
    Infer,
}

/// Attachment
#[derive(Debug)]
pub struct Attachment {
    /// Required. Filename
    pub(crate) filename: String,
    /// Required. MIME type, inferred when attached from URL
    pub(crate) mime_type: String,
    /// Required. Attachment content
    pub(crate) content: Vec<u8>,
}

impl Attachment {
    /// Creates an [`Attachment`]
    pub fn new(filename: &str, mime_type: &str, content: &[u8]) -> Self {
        Self {
            filename: filename.into(),
            mime_type: mime_type.into(),
            content: content.into(),
        }
    }

    /// Creates an [`Attachment`] with path
    pub async fn from_path(path: &Path) -> Result<Self, AttachmentError> {
        let mut buffer = Vec::new();
        let mut handle = File::open(path)?;
        handle.read_to_end(&mut buffer)?;
        let filename = path
            .file_name()
            .map_or("filename", |t| t.to_str().map_or("filename", |t| t));
        let mime_type = infer::get(&buffer).ok_or(AttachmentError::Infer)?;
        Ok(Self::new(filename, mime_type.mime_type(), &buffer))
    }

    /// Creates an [`Attachment`] with URL
    pub async fn from_url(url: &str) -> Result<Self, AttachmentError> {
        let parsed = Url::parse(url)?;
        let filename = parsed
            .path_segments()
            .map_or("filename", |t| t.last().map_or("filename", |t1| t1));

        let res = reqwest::get(url).await?;
        let buffer = res.bytes().await?.to_vec();

        let mime_type = infer::get(&buffer).ok_or(AttachmentError::Infer)?;

        Ok(Self {
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            content: buffer,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::server_url;
    use crate::{Attachment, AttachmentError};
    use mockito::mock;

    #[test]
    fn test_attachment_new() {
        Attachment::new("filename", "plain/text", &[]);
    }

    #[tokio::test]
    async fn test_attach_url() -> Result<(), AttachmentError> {
        let _n = mock("GET", "/filename.png")
            .with_status(200)
            .with_body(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
            .create();

        let u = format!("{}/filename.png", server_url());
        let a = Attachment::from_url(&u).await?;
        assert_eq!("filename.png", a.filename);
        assert_eq!("image/png", a.mime_type);
        assert!(a.content.len() > 0);
        Ok(())
    }
}
