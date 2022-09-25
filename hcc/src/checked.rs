use std::borrow::Cow;
use std::fmt;

use chrono::{DateTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString as _};

/// State of SSL certificate
#[derive(Debug)]
pub enum CertificateState {
    /// Default state
    NotChecked,
    /// Any error occurred when
    Error(anyhow::Error),
    /// Certificate is valid
    Ok {
        /// Remaining days to the expiration date
        days: i64,
        /// Exact expiration time in seconds since Unix epoch
        not_after: i64,
        /// Certificate will expire in grace period
        warned: bool,
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CertificateState::NotChecked => write!(f, "NOT_CHECKED"),
            CertificateState::Ok { warned, .. } => {
                if *warned {
                    write!(f, "WARNING")
                } else {
                    write!(f, "OK")
                }
            }
            CertificateState::Expired => write!(f, "EXPIRED"),
            CertificateState::Error(..) => write!(f, "ERROR"),
        }
    }
}

/// Check result
#[derive(Debug, Default)]
pub struct Checked<'a> {
    /// State of certificate
    pub state: CertificateState,
    /// ASCII only?
    pub ascii: bool,
    /// When is domain name got checked in seconds since Unix epoch
    pub checked_at: i64,
    /// Domain name that got checked
    pub domain_name: Cow<'a, str>,
    /// Elapsed time in milliseconds
    pub elapsed: Option<u128>,
}

impl<'a> Checked<'a> {
    /// Error occurred when checking
    ///
    /// ```
    /// # use anyhow::Error;
    /// use hcc::Checked;
    /// Checked::error("example.invalid", Error::msg("invalid DNS lookup"));
    /// ```
    pub fn error<T>(domain_name: T, e: anyhow::Error) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        Checked {
            state: CertificateState::Error(e),
            domain_name: domain_name.into(),
            ..Default::default()
        }
    }

    /// Create a result from expired domain name and when the check occurred
    ///
    /// ```
    /// # use hcc::Checked;
    /// use chrono::Utc;
    /// Checked::expired(false, "expired.badssl.com", &Utc::now());
    /// ```
    pub fn expired<T>(ascii: bool, domain_name: T, checked_at: &'a DateTime<Utc>) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        Checked {
            ascii,
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
    pub fn sentence(&self) -> Cow<'a, str> {
        let domain_name = &self.domain_name;
        match self.state {
            CertificateState::NotChecked => {
                let domain_name = &self.domain_name;
                format!("{domain_name} cert is unknown").into()
            }
            CertificateState::Ok {
                days, not_after, ..
            } => {
                let domain_name = &self.domain_name;
                let days = days.to_formatted_string(&Locale::en);
                let r = Utc.timestamp(not_after, 0).to_rfc3339();
                format!("{domain_name} cert expires in {days} days ({r})").into()
            }
            CertificateState::Expired => format!("{domain_name} cert expired").into(),
            CertificateState::Error(ref e) => format!("failed to check {domain_name}: {e}").into(),
        }
    }

    /// Icon of certificate state in ASCII or Unicode
    ///
    /// ```
    /// # use hcc::Checked;
    /// let result = Checked::default();
    /// result.state_icon();
    /// ```
    pub fn state_icon(&self) -> Cow<'a, str> {
        let s = match self.state {
            CertificateState::NotChecked => {
                if self.ascii {
                    "[?]"
                } else {
                    "\u{2753}"
                }
            }
            CertificateState::Ok { warned, .. } => {
                if warned {
                    if self.ascii {
                        "[-]"
                    } else {
                        "\u{26a0}\u{fe0f}"
                    }
                } else if self.ascii {
                    "[v]"
                } else {
                    "\u{2705}"
                }
            }
            CertificateState::Expired | CertificateState::Error(_) => {
                if self.ascii {
                    "[x]"
                } else {
                    "\u{274c}"
                }
            }
        };
        s.into()
    }
}

impl<'a> fmt::Display for Checked<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::with_capacity(100);
        s.push_str(&self.state_icon());
        s.push(' ');
        s.push_str(&self.sentence());
        if let Some(elapsed) = self.elapsed {
            s.push_str(&format!(", {elapsed}ms elapsed"));
        }
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use chrono::{Duration, SubsecRound as _, Utc};
    use std::ops::Add;

    fn build_checked<'a>() -> Checked<'a> {
        let now = Utc::now().round_subsecs(0);
        Checked {
            checked_at: now.timestamp(),
            domain_name: "badssl.com".into(),
            ..Default::default()
        }
    }

    #[test]
    fn t_display() {
        let days = 512;
        let not_after = Utc::now().round_subsecs(0).add(Duration::days(days));

        let mut result = build_checked();
        result.state = CertificateState::Ok {
            days,
            not_after: not_after.timestamp(),
            warned: false,
        };

        let left = format!("{result}");

        let r = not_after.to_rfc3339();
        let right = format!("\u{2705} badssl.com cert expires in {days} days ({r})",);
        assert_eq!(left, right);
    }

    #[test]
    fn t_display_warning() {
        let days = 512;
        let not_after = Utc::now().round_subsecs(0).add(Duration::days(days));

        let mut result = build_checked();
        result.state = CertificateState::Ok {
            days,
            not_after: not_after.timestamp(),
            warned: true,
        };
        let left = format!("{result}");

        let r = not_after.to_rfc3339();
        let right = format!("\u{26a0}\u{fe0f} badssl.com cert expires in {days} days ({r})");
        assert_eq!(left, right);
    }

    #[test]
    fn t_display_expired() {
        let mut result = build_checked();
        result.state = CertificateState::Expired;

        let left = format!("{result}");
        let right = "\u{274c} badssl.com cert expired";
        assert_eq!(left, right);
    }
}
