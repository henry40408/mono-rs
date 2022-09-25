use std::borrow::Cow;
use std::fmt;

use chrono::{DateTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString};

/// State of SSL certificate
#[derive(Debug)]
pub enum CertificateState {
    /// Default state
    NotChecked,
    /// Any error occurred when
    Error(String),
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
    /// use hcc::Checked;
    /// Checked::error("example.invalid", "invalid DNS lookup");
    /// ```
    pub fn error<T, U>(domain_name: T, e: U) -> Self
    where
        T: Into<Cow<'a, str>>,
        U: std::fmt::Display,
    {
        Checked {
            state: CertificateState::Error(e.to_string()),
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
    pub fn sentence(&self) -> String {
        let domain_name = &self.domain_name;
        match self.state {
            CertificateState::NotChecked => {
                let domain_name = &self.domain_name;
                format!("certificate state of {domain_name} is unknown")
            }
            CertificateState::Ok {
                days, not_after, ..
            } => {
                let domain_name = &self.domain_name;
                let days = days.to_formatted_string(&Locale::en);
                let r = Utc.timestamp(not_after, 0).to_rfc3339();
                format!("certificate of {domain_name} expires in {days} days ({r})")
            }
            CertificateState::Expired => format!("certificate of {domain_name} has expired"),
            CertificateState::Error(ref e) => {
                format!("failed to check {domain_name}: {e}")
            }
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
        s.to_string()
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

    use chrono::{Duration, SubsecRound, Utc};
    use std::ops::Add;

    fn build_checked<'a>() -> Checked<'a> {
        let now = Utc::now().round_subsecs(0);
        Checked {
            checked_at: now.timestamp(),
            domain_name: "example.com".into(),
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
        let right = format!("\u{2705} certificate of example.com expires in {days} days ({r})",);
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
        let r = format!("\u{26a0}\u{fe0f} certificate of example.com expires in {days} days ({r})");
        assert_eq!(left, r);
    }

    #[test]
    fn t_display_expired() {
        let mut result = build_checked();
        result.state = CertificateState::Expired;
        let left = format!("{result}");
        let right = "\u{274c} certificate of example.com has expired";
        assert_eq!(left, right);
    }
}
