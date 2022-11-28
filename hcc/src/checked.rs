use std::borrow::Cow;
use std::time::Duration;

use chrono::{DateTime, Utc};

/// Error or certificate information
#[derive(Debug)]
pub enum CheckedInner {
    /// An error occurred
    Error {
        /// Root cause
        error: anyhow::Error,
    },
    /// Certificate is valid
    Ok {
        /// Elapsed time checking
        elapsed: Duration,
        /// Expiration time
        not_after: DateTime<Utc>,
    },
}

/// Check result
#[derive(Debug)]
pub struct Checked<'a> {
    /// When is domain name checked
    pub checked_at: DateTime<Utc>,
    /// Domain name
    pub domain_name: Cow<'a, str>,
    /// Error or certificate information
    pub inner: CheckedInner,
}
