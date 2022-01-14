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
    Ok {
        /// Remaining days to the expiration date
        days: i64,
        /// Exact expiration time in seconds since Unix epoch
        not_after: i64,
    },
    /// Certificate is going to expire soon
    Warning {
        /// Remaining days to the expiration date
        days: i64,
        /// Exact expiration time in seconds since Unix epoch
        not_after: i64,
    },
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
            CertificateState::NotChecked => write!(f, "NOT_CHECKED"),
            CertificateState::Ok { .. } => write!(f, "OK"),
            CertificateState::Warning { .. } => write!(f, "WARNING"),
            CertificateState::Expired => write!(f, "EXPIRED"),
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
    /// Domain name that got checked
    pub domain_name: Cow<'a, str>,
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

    /// Human-readable sentence of certificate state
    ///
    /// ```
    /// # use hcc::Checked;
    /// let result = Checked::default();
    /// result.sentence();
    /// ```
    pub fn sentence(&self) -> String {
        match self.state {
            CertificateState::NotChecked => {
                format!("certificate state of {} is unknown", self.domain_name)
            }
            CertificateState::Ok { days, not_after } => format!(
                "certificate of {} expires in {} days ({})",
                self.domain_name,
                days.to_formatted_string(&Locale::en),
                Utc.timestamp(not_after, 0).to_rfc3339()
            ),
            CertificateState::Warning { days, not_after } => format!(
                "certificate of {} expires in {} days ({})",
                self.domain_name,
                days.to_formatted_string(&Locale::en),
                Utc.timestamp(not_after, 0).to_rfc3339()
            ),
            CertificateState::Expired => format!("certificate of {} has expired", self.domain_name),
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
            CertificateState::Ok { .. } => {
                if self.ascii {
                    "[v]"
                } else {
                    "\u{2705}"
                }
            }
            CertificateState::Warning { .. } => {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
    /// Domain name that got checked
    pub domain_name: String,
    /// Expiration time in RFC3389 format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired_at: Option<String>,
    /// Elapsed time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<u128>,
}

impl CheckedJSON {
    /// Convert result to JSON
    ///
    /// ```
    /// # use hcc::{Checked, CheckedJSON};
    /// use chrono::Utc;
    /// let checked = Checked {
    ///     domain_name: "sha512.badssl.com".into(),
    ///     checked_at: Utc::now().timestamp(),
    ///     ..Default::default()
    /// };
    /// CheckedJSON::new(&checked);
    /// ```
    pub fn new(checked: &Checked) -> CheckedJSON {
        CheckedJSON {
            state: checked.state.to_string(),
            days: match checked.state {
                CertificateState::NotChecked | CertificateState::Expired => None,
                CertificateState::Ok { days, not_after: _ } => Some(days),
                CertificateState::Warning { days, not_after: _ } => Some(days),
            },
            domain_name: checked.domain_name.to_string(),
            checked_at: Utc.timestamp(checked.checked_at, 0).to_rfc3339(),
            expired_at: match checked.state {
                CertificateState::NotChecked | CertificateState::Expired => None,
                CertificateState::Ok { days: _, not_after } => {
                    Some(Utc.timestamp(not_after, 0).to_rfc3339())
                }
                CertificateState::Warning { days: _, not_after } => {
                    Some(Utc.timestamp(not_after, 0).to_rfc3339())
                }
            },
            elapsed: checked.elapsed,
        }
    }
}

#[cfg(test)]
mod test {
    use chrono::{Duration, SubsecRound, Utc};
    use std::ops::Add;

    use crate::checked::CertificateState;
    use crate::Checked;

    fn build_checked<'a>() -> Checked<'a> {
        let now = Utc::now().round_subsecs(0);
        Checked {
            checked_at: now.timestamp(),
            domain_name: "example.com".into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_display() {
        let days = 512;
        let not_after = Utc::now().round_subsecs(0).add(Duration::days(days));

        let mut result = build_checked();
        result.state = CertificateState::Ok {
            days,
            not_after: not_after.timestamp(),
        };

        let left = format!("{}", result);
        let right = format!(
            "\u{2705} certificate of example.com expires in {} days ({})",
            days,
            not_after.to_rfc3339()
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_display_warning() {
        let days = 512;
        let not_after = Utc::now().round_subsecs(0).add(Duration::days(days));

        let mut result = build_checked();
        result.state = CertificateState::Warning {
            days,
            not_after: not_after.timestamp(),
        };
        let left = format!("{}", result);
        let right = format!(
            "\u{26a0}\u{fe0f} certificate of example.com expires in {} days ({})",
            days,
            not_after.to_rfc3339()
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_display_expired() {
        let mut result = build_checked();
        result.state = CertificateState::Expired;
        let left = format!("{}", result);
        let right = format!("\u{274c} certificate of example.com has expired");
        assert_eq!(left, right);
    }
}
