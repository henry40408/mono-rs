use std::borrow::Cow;
use std::convert::TryFrom;
use std::fmt::Formatter;
use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context};
use chrono::{DateTime, SubsecRound, TimeZone, Utc};
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

impl std::fmt::Debug for Checker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CheckClient {{ checked_at: {:?}, elapsed: {:?}, grace_in_days: {:?} }}",
            self.checked_at, self.elapsed, self.grace_in_days
        )
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

        let config = rustls::ClientConfig::builder()
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
    /// client.check_one("sha512.badssl.com");
    /// client.check_one("sha512.badssl.com".to_string());
    /// ```
    pub async fn check_one<'a, T>(&'a self, domain_name: T) -> anyhow::Result<Checked<'a>>
    where
        T: Into<Cow<'a, str>>,
    {
        let domain_name = domain_name.into();
        let server_name = match ServerName::try_from(domain_name.as_ref()) {
            Ok(s) => s,
            Err(_e) => bail!("invalid domain name"),
        };
        let mut conn = rustls::ClientConnection::new(self.config.clone(), server_name)?;
        let mut stream = TcpStream::connect(format!("{0}:443", domain_name.as_ref()))?;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);

        let start = Instant::now();
        match tls.write(Self::build_http_headers(domain_name.as_ref()).as_bytes()) {
            Ok(_) => (),
            Err(_) => return Ok(Checked::expired(domain_name.to_owned(), &self.checked_at)),
        };
        let elapsed = start.elapsed();

        let certificates = tls
            .conn
            .peer_certificates()
            .with_context(|| format!("no peer certificates found for {0}", domain_name.as_ref()))?;

        let certificate = certificates
            .first()
            .with_context(|| format!("no certificate found for {0}", domain_name.as_ref()))?;

        let not_after = match parse_x509_certificate(certificate.as_ref()) {
            Ok((_, cert)) => cert.validity().not_after,
            Err(_) => return Ok(Checked::default()),
        };
        let not_after = Utc.timestamp(not_after.timestamp(), 0);

        let duration = not_after - self.checked_at;
        let days = duration.num_days();
        let not_after = not_after.timestamp();
        let state = if days > self.grace_in_days {
            CertificateState::Ok { days, not_after }
        } else {
            CertificateState::Warning { days, not_after }
        };
        Ok(Checked {
            state,
            ascii: self.ascii,
            checked_at: self.checked_at.timestamp(),
            domain_name: domain_name.to_owned(),
            elapsed: if self.elapsed {
                Some(elapsed.as_millis())
            } else {
                None
            },
        })
    }

    /// Check SSL certificates of multiple domain names
    ///
    /// ```
    /// # use hcc::Checker;
    /// let client = Checker::default();
    /// client.check_many(&["sha256.badssl.com", "sha256.badssl.com"]);
    /// client.check_many(&["sha256.badssl.com".to_string(), "sha256.badssl.com".to_string()]);
    /// ```
    pub async fn check_many<'a, T>(
        &'a self,
        domain_names: &'a [T],
    ) -> anyhow::Result<Vec<Checked<'a>>>
    where
        T: AsRef<str>,
    {
        let client = Arc::new(self);

        let mut tasks = vec![];
        for domain_name in domain_names {
            let client = client.clone();
            tasks.push(client.check_one(domain_name.as_ref()));
        }

        Ok(futures::future::try_join_all(tasks).await?)
    }

    fn build_http_headers<T>(domain_name: T) -> String
    where
        T: AsRef<str>,
    {
        format!(
            concat!(
                "GET / HTTP/1.1\r\n",
                "Host: {0}\r\n",
                "Connection: close\r\n",
                "Accept-Encoding: identity\r\n",
                "\r\n"
            ),
            domain_name.as_ref()
        )
    }
}

#[cfg(test)]
mod test {
    use crate::checked::CertificateState;
    use crate::checker::Checker;

    #[tokio::test]
    async fn test_good_certificate() {
        let client = Checker::default();

        let result = client.check_one("sha512.badssl.com").await.unwrap();
        assert!(matches!(result.state, CertificateState::Ok { .. }));
        assert!(result.checked_at > 0);
        if let CertificateState::Ok { days, not_after } = result.state {
            assert!(days > 0);
            assert!(not_after > 0);
        }

        let _result = client
            .check_one("sha512.badssl.com".to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_bad_certificate() {
        let client = Checker::default();
        let result = client.check_one("expired.badssl.com").await.unwrap();
        assert!(matches!(result.state, CertificateState::Expired));
        assert!(result.checked_at > 0);
    }

    #[tokio::test]
    async fn test_check_many() -> anyhow::Result<()> {
        let domain_names = vec!["sha512.badssl.com", "expired.badssl.com"];
        let client = Checker::default();

        let results = client.check_many(domain_names.as_slice()).await?;
        assert_eq!(2, results.len());

        let result = results.get(0).unwrap();
        assert!(matches!(result.state, CertificateState::Ok { .. }));

        let result = results.get(1).unwrap();
        assert!(matches!(result.state, CertificateState::Expired));

        Ok(())
    }

    #[tokio::test]
    async fn test_check_many_with_grace_in_days() {
        let domain_name = "sha512.badssl.com";

        let client = Checker::default();
        let result = client.check_one(domain_name).await.unwrap();
        assert!(matches!(result.state, CertificateState::Ok { .. }));

        if let CertificateState::Ok { days, not_after: _ } = result.state {
            let mut client = Checker::default();
            client.grace_in_days = days + 1;

            let result = client.check_one(domain_name).await.unwrap();
            assert!(matches!(result.state, CertificateState::Warning { .. }));
        }
    }
}
