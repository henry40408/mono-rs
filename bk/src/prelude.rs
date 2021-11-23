use chrono::{DateTime, NaiveDateTime, Utc};

/// Provides RFC3339 method to columns e.g. created_at
pub trait RFC3339Ext {
    /// Return timestamp in RFC3339 format
    fn rfc3339(&self) -> String;
}

impl RFC3339Ext for NaiveDateTime {
    fn rfc3339(&self) -> String {
        let dt = DateTime::<Utc>::from_utc(*self, Utc);
        dt.to_rfc3339()
    }
}
