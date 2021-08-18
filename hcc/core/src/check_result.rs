use std::fmt;

use chrono::{DateTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString};
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;

/// State of Certificate
#[derive(Debug)]
pub enum CheckState {
    /// Default state
    Unknown,
    /// Certificate is valid
    Ok,
    /// Certificate is going to expire soon
    Warning,
    /// Certificate expired
    Expired,
}

impl Default for CheckState {
    fn default() -> Self {
        CheckState::Unknown
    }
}

impl fmt::Display for CheckState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CheckState::Unknown => write!(f, "Unknown"),
            CheckState::Ok => write!(f, "OK"),
            CheckState::Warning => write!(f, "WARNING"),
            CheckState::Expired => write!(f, "EXPIPRED"),
        }
    }
}

/// Check result
#[derive(Debug, Default)]
pub struct CheckResult<'a> {
    /// State of certificate
    pub state: CheckState,
    /// When is domain name got checked in seconds since Unix epoch
    pub checked_at: i64,
    /// Remaining days to the expiration date
    pub days: i64,
    /// Domain name that got checked
    pub domain_name: &'a str,
    /// Exact expiration time in seconds since Unix epoch
    pub not_after: i64,
    /// Elapsed time in milliseconds
    pub elapsed: Option<u128>,
}

impl<'a> CheckResult<'a> {
    /// Create a result from expired domain name and when the check occurred
    ///
    /// ```
    /// # use hcc::CheckResult;
    /// use chrono::Utc;
    /// CheckResult::expired("expired.badssl.com", &Utc::now());
    /// ```
    pub fn expired(domain_name: &'a str, checked_at: &'a DateTime<Utc>) -> Self {
        CheckResult {
            state: CheckState::Expired,
            checked_at: checked_at.timestamp(),
            domain_name,
            ..Default::default()
        }
    }

    /// Expiration date of certficate in RFC3339 format
    ///
    /// ```
    /// # use hcc::CheckResult;
    /// let result = CheckResult::default();
    /// result.not_after_timestamp();
    /// ```
    pub fn not_after_timestamp(&self) -> String {
        Utc.timestamp(self.not_after, 0).to_rfc3339()
    }

    /// Human-readable sentence of certificate state
    ///
    /// ```
    /// # use hcc::CheckResult;
    /// let result = CheckResult::default();
    /// result.sentence();
    /// ```
    pub fn sentence(&self) -> String {
        let days = self.days.to_formatted_string(&Locale::en);
        match self.state {
            CheckState::Unknown => format!("certificate state of {} is unknown", self.domain_name),
            CheckState::Ok => format!(
                "certificate of {} expires in {} days ({})",
                self.domain_name,
                days,
                self.not_after_timestamp()
            ),
            CheckState::Warning => format!(
                "certificate of {} expires in {} days ({})",
                self.domain_name,
                days,
                self.not_after_timestamp()
            ),
            CheckState::Expired => format!(
                "certificate of {} has expired ({})",
                self.domain_name,
                self.not_after_timestamp()
            ),
        }
    }

    /// Icon of certificate state in ASCII or Unicode
    ///
    /// ```
    /// # use hcc::CheckResult;
    /// let result = CheckResult::default();
    /// result.state_icon(true);
    /// result.state_icon(false);
    /// ```
    pub fn state_icon(&self, unicode: bool) -> String {
        let s = match self.state {
            CheckState::Unknown => {
                if unicode {
                    "\u{2753}"
                } else {
                    "[?]"
                }
            }
            CheckState::Ok => {
                if unicode {
                    "\u{2705}"
                } else {
                    "[v]"
                }
            }
            CheckState::Warning => {
                if unicode {
                    "\u{26a0}\u{fe0f}"
                } else {
                    "[-]"
                }
            }
            CheckState::Expired => {
                if unicode {
                    "\u{274c}"
                } else {
                    "[x]"
                }
            }
        };
        s.to_string()
    }
}

impl<'a> fmt::Display for CheckResult<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::with_capacity(100);

        s.push_str(&self.state_icon(false));

        s.push_str(&" ");

        s.push_str(&self.sentence());

        if let Some(elapsed) = self.elapsed {
            s.push_str(&format!(", {0}ms elapsed", elapsed));
        }

        write!(f, "{}", s)
    }
}

/// Check result in JSON format
#[derive(Default, Serialize, Deserialize)]
pub struct CheckResultJSON {
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

impl CheckResultJSON {
    /// Convert result to JSON
    ///
    /// ```
    /// # use hcc::{CheckResult, CheckResultJSON};
    /// use chrono::Utc;
    /// let result = CheckResult {
    ///     domain_name: "sha512.badssl.com".into(),
    ///     checked_at: Utc::now().timestamp(),
    ///     ..Default::default()
    /// };
    /// CheckResultJSON::new(&result);
    /// ```
    pub fn new(result: &CheckResult) -> CheckResultJSON {
        CheckResultJSON {
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

    use crate::check_result::CheckState;
    use crate::CheckResult;

    fn build_result<'a>() -> CheckResult<'a> {
        let days = 512;
        let now = Utc::now().round_subsecs(0);
        let expired_at = now + Duration::days(days);
        CheckResult {
            checked_at: now.timestamp(),
            days,
            domain_name: &"example.com",
            not_after: expired_at.timestamp(),
            ..Default::default()
        }
    }

    #[test]
    fn test_display() {
        let mut result = build_result();
        result.state = CheckState::Ok;

        let left = format!("{0}", result);
        let right = format!(
            "[v] certificate of example.com expires in 512 days ({0})",
            Utc.timestamp(result.not_after, 0).to_rfc3339()
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_display_warning() {
        let mut result = build_result();
        result.state = CheckState::Warning;
        let left = format!("{0}", result);
        let right = format!(
            "[-] certificate of example.com expires in 512 days ({0})",
            Utc.timestamp(result.not_after, 0).to_rfc3339()
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_display_expired() {
        let mut result = build_result();
        result.state = CheckState::Expired;
        let left = format!("{0}", result);
        let right = format!(
            "[x] certificate of example.com has expired ({})",
            Utc.timestamp(result.not_after, 0).to_rfc3339()
        );
        assert_eq!(left, right);
    }
}
