use std::borrow::Cow;
use std::fmt;
use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use anyhow::Context as _;
use chrono::{TimeZone, Utc};
use futures::stream::FuturesOrdered;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, ClientConfig, OwnedTrustAnchor, ServerName};
use x509_parser::parse_x509_certificate;

use crate::checked::Checked;
use crate::CheckedInner;

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

fn do_check_one<'a, T>(config: Arc<ClientConfig>, domain_name: T) -> anyhow::Result<Checked<'a>>
where
    T: Into<Cow<'a, str>>,
{
    use anyhow::Error;

    let now = Utc::now();

    let domain_name = domain_name.into();
    let server_name = ServerName::try_from(domain_name.as_ref())?;
    let mut conn = rustls::ClientConnection::new(config, server_name)?;

    let mut stream = TcpStream::connect(format!("{domain_name}:443"))?;
    let mut tls = rustls::Stream::new(&mut conn, &mut stream);

    let start = Instant::now();
    let _ = tls.write(build_http_headers(domain_name.as_ref()).as_bytes());

    let certificates = tls
        .conn
        .peer_certificates()
        .context("no peer certificates found")?;

    let certificate = certificates.first().context("no peer certificate found")?;

    let (_, cert) = parse_x509_certificate(certificate.as_ref())?;
    let not_after = match Utc
        .timestamp_opt(cert.validity().not_after.timestamp(), 0)
        .single()
    {
        Some(t) => t,
        None => return Err(Error::msg("invalid timestamp")),
    };
    Ok(Checked {
        checked_at: now,
        domain_name,
        inner: CheckedInner::Ok {
            elapsed: start.elapsed(),
            not_after,
        },
    })
}

struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: SystemTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
}

/// Checker for SSL certificate
pub struct Checker {
    config: Arc<ClientConfig>,
}

impl fmt::Debug for Checker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checker").finish()
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
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();

        Checker {
            config: Arc::new(config),
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
        let config = self.config.clone();
        match do_check_one(config, domain_name.clone()) {
            Ok(c) => c,
            Err(error) => Checked {
                checked_at: Utc::now(),
                domain_name: domain_name.into(),
                inner: CheckedInner::Error { error },
            },
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
    pub async fn check_many<'a, T>(
        &'a self,
        domain_names: &'a [T],
    ) -> anyhow::Result<Vec<Checked<'a>>>
    where
        T: AsRef<str>,
    {
        use futures::StreamExt as _;

        let now = Utc::now();

        let mut tasks = FuturesOrdered::new();
        for domain_name in domain_names {
            let config = self.config.clone();
            let domain_name = domain_name.as_ref().to_string();
            tasks.push_back(tokio::spawn(async move {
                match do_check_one(config, domain_name.clone()) {
                    Ok(c) => c,
                    Err(error) => Checked {
                        checked_at: now,
                        domain_name: domain_name.into(),
                        inner: CheckedInner::Error { error },
                    },
                }
            }));
        }

        let mut results = vec![];
        while let Some(task) = tasks.next().await {
            results.push(task?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn t_good_certificate() {
        let client = Checker::default();
        let checked = client.check_one("sha256.badssl.com").await;
        assert!(matches!(checked.inner, CheckedInner::Ok { .. }));
        if let CheckedInner::Ok { not_after, .. } = checked.inner {
            assert!(not_after > checked.checked_at);
        }
    }

    #[tokio::test]
    async fn t_bad_certificate() {
        let client = Checker::default();
        let checked = client.check_one("expired.badssl.com").await;
        assert!(matches!(checked.inner, CheckedInner::Ok { .. }));
        if let CheckedInner::Ok { not_after, .. } = checked.inner {
            assert!(not_after < checked.checked_at);
        }
    }

    #[tokio::test]
    async fn t_check_many() {
        let domain_names = vec!["sha256.badssl.com", "expired.badssl.com"];
        let client = Checker::default();

        let results = client.check_many(domain_names.as_slice()).await.unwrap();
        assert_eq!(2, results.len());

        let result = &results[0];
        assert!(matches!(result.inner, CheckedInner::Ok { .. }));
        if let CheckedInner::Ok { not_after, .. } = result.inner {
            assert!(not_after > result.checked_at);
        }

        let result = &results[1];
        assert!(matches!(result.inner, CheckedInner::Ok { .. }));
        if let CheckedInner::Ok { not_after, .. } = result.inner {
            assert!(not_after < result.checked_at);
        }
    }

    #[tokio::test]
    async fn t_check_one_invalid() {
        let client = Checker::default();
        let result = client.check_one("example.invalid").await;
        assert!(matches!(result.inner, CheckedInner::Error { .. }));
    }
}
