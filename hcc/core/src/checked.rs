use std::borrow::Cow;
use std::fmt;
use std::fmt::Formatter;

use chrono::{DateTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString};
use serde::{Deserialize, Serialize};

/// State of SSL certificate
#[derive(Clone, Copy, Debug)]
pub enum CertificateState {
    /// Default state
    NotChecked,
    /// Certificate is valid
    Ok,
    /// Certificate is going to expire soon
    Warning,
    /// Certificate expired
    Expired,
}

impl Default for CertificateState {
    fn default() -> Self {
        CertificateState::NotChecked
    }
}

impl fmt::Display for CertificateState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CertificateState::NotChecked => write!(f, "not checked"),
            CertificateState::Ok => write!(f, "OK"),
            CertificateState::Warning => write!(f, "warning"),
            CertificateState::Expired => write!(f, "expired"),
        }
    }
}

/// Check result
#[derive(Debug, Default)]
pub struct Checked<'a> {
    /// State of certificate
    pub state: CertificateState,
    /// ASCII?
    pub ascii: bool,
    /// When is domain name got checked in seconds since Unix epoch
    pub checked_at: i64,
    /// Remaining days to the expiration date
    pub days: i64,
    /// Domain name that got checked
    pub domain_name: Cow<'a, str>,
    /// Exact expiration time in seconds since Unix epoch
    pub not_after: i64,
    /// Elapsed time in milliseconds
    pub elapsed: Option<u128>,
}

impl<'a> Checked<'a> {
    /// Create a result from expired domain name and when the check occurred
    ///
    /// ```
    /// # use hcc::Checked;
    /// use chrono::Utc;
    /// Checked::expired("expired.badssl.com", &Utc::now());
    /// ```
    pub fn expired<T>(domain_name: T, checked_at: &'a DateTime<Utc>) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        Checked {
            state: CertificateState::Expired,
            checked_at: checked_at.timestamp(),
            domain_name: domain_name.into(),
            ..Default::default()
        }
    }

    /// Expiration date of certficate in RFC3339 format
    ///
    /// ```
    /// # use hcc::Checked;
    /// let result = Checked::default();
    /// result.not_after_timestamp();
    /// ```
    pub fn not_after_timestamp(&self) -> String {
        Utc.timestamp(self.not_after, 0).to_rfc3339()
    }

    /// Human-readable sentence of certificate state
    ///
    /// ```
    /// # use hcc::Checked;
    /// let result = Checked::default();
    /// result.sentence();
    /// ```
    pub fn sentence(&self) -> String {
        let days = self.days.to_formatted_string(&Locale::en);
        match self.state {
            CertificateState::NotChecked => {
                format!("certificate state of {} is unknown", self.domain_name)
            }
            CertificateState::Ok => format!(
                "certificate of {} expires in {} days ({})",
                self.domain_name,
                days,
                self.not_after_timestamp()
            ),
            CertificateState::Warning => format!(
                "certificate of {} expires in {} days ({})",
                self.domain_name,
                days,
                self.not_after_timestamp()
            ),
            CertificateState::Expired => format!(
                "certificate of {} has expired ({})",
                self.domain_name,
                self.not_after_timestamp()
            ),
        }
    }

    /// Icon of certificate state in ASCII or Unicode
    ///
    /// ```
    /// # use hcc::Checked;
    /// let result = Checked::default();
    /// result.state_icon();
    /// ```
    pub fn state_icon(&self) -> String {
        let s = match self.state {
            CertificateState::NotChecked => {
                if self.ascii {
                    "[?]"
                } else {
                    "\u{2753}"
                }
            }
            CertificateState::Ok => {
                if self.ascii {
                    "[v]"
                } else {
                    "\u{2705}"
                }
            }
            CertificateState::Warning => {
                if self.ascii {
                    "[-]"
                } else {
                    "\u{26a0}\u{fe0f}"
                }
            }
            CertificateState::Expired => {
                if self.ascii {
                    "[x]"
                } else {
                    "\u{274c}"
                }
            }
        };
        s.to_string()
    }
}

impl<'a> fmt::Display for Checked<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut s = String::with_capacity(100);

        s.push_str(&self.state_icon());

        s.push(' ');

        s.push_str(&self.sentence());

        if let Some(elapsed) = self.elapsed {
            s.push_str(&format!(", {0}ms elapsed", elapsed));
        }

        write!(f, "{}", s)
    }
}

/// Check result in JSON format
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CheckedJSON {
    /// State of certificate
    pub state: String,
    /// When is the domain name got checked
    pub checked_at: String,
    /// Remaining days to the expiration date
    pub days: i64,
    /// Domain name that got checked
    pub domain_name: String,
    /// Expiration time in RFC3389 format
    pub expired_at: String,
    /// Elapsed time in milliseconds
    pub elapsed: u128,
}

impl CheckedJSON {
    /// Convert result to JSON
    ///
    /// ```
    /// # use hcc::{Checked, CheckedJSON};
    /// use chrono::Utc;
    /// let result = Checked {
    ///     domain_name: "sha512.badssl.com".into(),
    ///     checked_at: Utc::now().timestamp(),
    ///     ..Default::default()
    /// };
    /// CheckedJSON::new(&result);
    /// ```
    pub fn new(result: &Checked) -> CheckedJSON {
        CheckedJSON {
            state: result.state.to_string(),
            days: result.days,
            domain_name: result.domain_name.to_string(),
            checked_at: Utc.timestamp(result.checked_at, 0).to_rfc3339(),
            expired_at: Utc.timestamp(result.not_after, 0).to_rfc3339(),
            elapsed: result.elapsed.unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod test {
    use chrono::{Duration, SubsecRound, TimeZone, Utc};

    use crate::checked::CertificateState;
    use crate::Checked;

    fn build_result<'a>() -> Checked<'a> {
        let days = 512;
        let now = Utc::now().round_subsecs(0);
        let expired_at = now + Duration::days(days);
        Checked {
            checked_at: now.timestamp(),
            days,
            domain_name: "example.com".into(),
            not_after: expired_at.timestamp(),
            ..Default::default()
        }
    }

    #[test]
    fn test_display() {
        let mut result = build_result();
        result.state = CertificateState::Ok;

        let left = format!("{0}", result);
        let right = format!(
            "\u{2705} certificate of example.com expires in 512 days ({0})",
            Utc.timestamp(result.not_after, 0).to_rfc3339()
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_display_warning() {
        let mut result = build_result();
        result.state = CertificateState::Warning;
        let left = format!("{0}", result);
        let right = format!(
            "\u{26a0}\u{fe0f} certificate of example.com expires in 512 days ({0})",
            Utc.timestamp(result.not_after, 0).to_rfc3339()
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_display_expired() {
        let mut result = build_result();
        result.state = CertificateState::Expired;
        let left = format!("{0}", result);
        let right = format!(
            "\u{274c} certificate of example.com has expired ({})",
            Utc.timestamp(result.not_after, 0).to_rfc3339()
        );
        assert_eq!(left, right);
    }
}
