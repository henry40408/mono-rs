use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Instant;
use std::{borrow::Cow, fmt};

use anyhow::Context as _;
use chrono::{DateTime, SubsecRound as _, TimeZone, Utc};
use rustls::{ClientConfig, OwnedTrustAnchor, ServerName};
use x509_parser::parse_x509_certificate;

use crate::checked::{CertificateState, Checked};

/// Checker for SSL certificate
pub struct Checker {
    checked_at: DateTime<Utc>,
    config: Arc<ClientConfig>,
    /// ASCII only?
    pub ascii: bool,
    /// Show elapsed time in milliseconds?
    pub elapsed: bool,
    /// Grace period before certificate actually expires
    pub grace_in_days: i64,
}

impl fmt::Debug for Checker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checker")
            .field("checked_at", &self.checked_at)
            .field("ascii", &self.ascii)
            .field("elapsed", &self.elapsed)
            .field("grace_in_days", &self.grace_in_days)
            .finish()
    }
}

impl Default for Checker {
    fn default() -> Checker {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
            OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));

        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Checker {
            ascii: false,
            checked_at: Utc::now().round_subsecs(0),
            config: Arc::new(config),
            elapsed: false,
            grace_in_days: 7,
        }
    }
}

impl Checker {
    /// Check SSL certificate of one domain name
    ///
    /// ```
    /// # use hcc::Checker;
    /// let client = Checker::default();
    /// client.check_one("sha256.badssl.com");
    /// client.check_one("sha256.badssl.com".to_string());
    /// ```
    pub async fn check_one<'a, T>(&'a self, domain_name: T) -> Checked<'a>
    where
        T: Into<Cow<'a, str>> + Clone,
    {
        match self.do_check_one(domain_name.clone()) {
            Ok(c) => c,
            Err(e) => Checked::error(domain_name, e),
        }
    }

    /// Check SSL certificates of multiple domain names
    ///
    /// ```
    /// # use hcc::Checker;
    /// let client = Checker::default();
    /// client.check_many(&["sha256.badssl.com", "sha256.badssl.com"]);
    /// client.check_many(&["sha256.badssl.com".to_string(), "sha256.badssl.com".to_string()]);
    /// ```
    pub async fn check_many<'a, T>(&'a self, domain_names: &'a [T]) -> Vec<Checked<'a>>
    where
        T: AsRef<str>,
    {
        let client = Arc::new(self);
        let mut tasks = vec![];
        for domain_name in domain_names {
            let client = client.clone();
            tasks.push(client.check_one(domain_name.as_ref()));
        }
        futures::future::join_all(tasks).await
    }

    fn build_http_headers<'a, T>(domain_name: T) -> Cow<'a, str>
    where
        T: AsRef<str>,
    {
        let domain_name = domain_name.as_ref();
        format!(
            "GET / HTTP/1.1\r\n\
            Host: {domain_name}\r\n\
            Connection: close\r\n\
            Accept-Encoding: identity\r\n\
            \r\n"
        )
        .into()
    }

    fn do_check_one<'a, T>(&'a self, domain_name: T) -> anyhow::Result<Checked<'a>>
    where
        T: Into<Cow<'a, str>>,
    {
        let domain_name = domain_name.into();
        let server_name = ServerName::try_from(domain_name.as_ref())?;
        let mut conn = rustls::ClientConnection::new(self.config.clone(), server_name)?;

        let mut stream = TcpStream::connect(format!("{domain_name}:443"))?;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);

        let start = Instant::now();
        match tls.write(Self::build_http_headers(domain_name.as_ref()).as_bytes()) {
            Ok(_) => (),
            Err(_e) => return Ok(Checked::expired(self.ascii, domain_name, &self.checked_at)),
        };
        let elapsed = start.elapsed();

        let certificates = tls
            .conn
            .peer_certificates()
            .context("no peer certificates found")?;

        let certificate = certificates.first().context("no peer certificate found")?;

        let (_, cert) = parse_x509_certificate(certificate.as_ref())?;
        let not_after = Utc.timestamp(cert.validity().not_after.timestamp(), 0);

        let days = (not_after - self.checked_at).num_days();
        let not_after = not_after.timestamp();
        let warned = days < self.grace_in_days;
        Ok(Checked {
            state: CertificateState::Ok {
                days,
                not_after,
                warned,
            },
            ascii: self.ascii,
            checked_at: self.checked_at.timestamp(),
            domain_name,
            elapsed: if self.elapsed {
                Some(elapsed.as_millis())
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn t_good_certificate() {
        let mut client = Checker::default();
        client.grace_in_days = 1;

        let result = client.check_one("sha256.badssl.com").await;
        assert!(matches!(result.state, CertificateState::Ok { .. }));
        assert!(result.checked_at > 0);
        if let CertificateState::Ok {
            days,
            not_after,
            warned,
        } = result.state
        {
            assert!(days > 0);
            assert!(not_after > 0);
            assert!(!warned);
        }
    }

    #[tokio::test]
    async fn t_bad_certificate() {
        let mut client = Checker::default();
        client.grace_in_days = 1;

        let result = client.check_one("expired.badssl.com").await;
        assert!(matches!(result.state, CertificateState::Expired));
        assert!(result.checked_at > 0);
    }

    #[tokio::test]
    async fn t_check_many() -> anyhow::Result<()> {
        let domain_names = vec!["sha256.badssl.com", "expired.badssl.com"];

        let mut client = Checker::default();
        client.grace_in_days = 1;

        let results = client.check_many(domain_names.as_slice()).await;
        assert_eq!(2, results.len());

        let result = &results[0];
        dbg!(&result);
        assert!(matches!(result.state, CertificateState::Ok { .. }));

        let result = &results[1];
        assert!(matches!(result.state, CertificateState::Expired));

        Ok(())
    }

    #[tokio::test]
    async fn t_check_many_with_grace_in_days() {
        let domain_name = "sha256.badssl.com";

        let mut client = Checker::default();
        client.grace_in_days = 1;

        let result = client.check_one(domain_name).await;
        assert!(matches!(result.state, CertificateState::Ok { .. }));

        if let CertificateState::Ok { days, .. } = result.state {
            let client = Checker {
                grace_in_days: days + 1,
                ..Default::default()
            };
            let result = client.check_one(domain_name).await;
            assert!(matches!(result.state, CertificateState::Ok { .. }));

            if let CertificateState::Ok { warned, .. } = result.state {
                assert!(warned);
            }
        }
    }

    #[tokio::test]
    async fn t_check_one_invalid() {
        let client = Checker::default();
        let result = client.check_one("example.invalid").await;
        assert!(matches!(result.state, CertificateState::Error(..)));
    }
}
