use std::borrow::Cow;
use std::time::Duration;

use chrono::{DateTime, Utc};

/// Check result
#[derive(Debug, Default)]
pub struct Checked<'a> {
    /// When is domain name checked
    pub checked_at: DateTime<Utc>,
    /// Domain name
    pub domain_name: Cow<'a, str>,
    /// Elapsed time in milliseconds
    pub elapsed: Option<Duration>,
    /// Root cause
    pub error: Option<anyhow::Error>,
    /// Timestamp of expiration time, none when the certificate expired
    pub not_after: Option<DateTime<Utc>>,
}
