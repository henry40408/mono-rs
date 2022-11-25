#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

//! Cloudflare DNS record update.

use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use cloudflare::endpoints::dns::{DnsContent, DnsRecord};
use cloudflare::endpoints::zone::Zone;
use cloudflare::framework::response::ApiSuccess;
use log::{debug, Level};
use logging_timer::{finish, stimer};
use moka::sync::Cache;
use ureq::{Agent, AgentBuilder};

const HTTP_TIMEOUT: u64 = 30;

#[cfg(not(test))]
fn server_url() -> String {
    "https://api.cloudflare.com".to_string()
}

#[cfg(test)]
fn server_url() -> String {
    mockito::server_url()
}

/// Cannot fetch public IPv4 address
#[derive(Clone, Copy, Debug)]
pub struct NoIPV4;

impl Display for NoIPV4 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot fetch public IPv4 address")
    }
}

impl std::error::Error for NoIPV4 {}

#[derive(Eq, PartialEq, Hash)]
enum CacheKey {
    LastIP,
}

#[derive(Clone)]
enum Cached {
    IP(Ipv4Addr),
}

impl Display for Cached {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Cached::IP(i) => write!(f, "{i}"),
        }
    }
}

/// Cloudflare DNS Update
pub struct Cdu<'a> {
    token: Cow<'a, str>,
    zone: Cow<'a, str>,
    record_names: Vec<String>,
    cache: Cache<CacheKey, Cached>,
}

impl<'a> std::fmt::Debug for Cdu<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cdu")
            .field("token", &self.token)
            .field("zone", &self.zone)
            .field("record_names", &self.record_names)
            .finish()
    }
}

impl<'a> Cdu<'a> {
    /// Creates a [`Cdu`]
    pub fn new<T, U>(token: T, zone: T, record_names: &'a [U]) -> Self
    where
        T: Into<Cow<'a, str>>,
        U: Display,
    {
        Self {
            token: token.into(),
            zone: zone.into(),
            record_names: record_names
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
            cache: Cache::new(1), // cache IP address
        }
    }

    fn build_agent(&self) -> Agent {
        AgentBuilder::new()
            .timeout(Duration::from_secs(HTTP_TIMEOUT))
            .build()
    }

    async fn get_record_identifiers<T>(
        &self,
        agent: Arc<Agent>,
        zone_id: T,
        record_names: &[String],
    ) -> anyhow::Result<Vec<(String, String)>>
    where
        T: Into<Cow<'a, str>>,
    {
        let zone_id = zone_id.into();
        let authorization = format!("bearer {}", &self.token);

        let url = format!("{}/client/v4/zones/{zone_id}/dns_records", server_url());
        let req = agent
            .get(&url)
            .set("content-type", "application/json")
            .set("authorization", &authorization);
        let tmr = stimer!(Level::Debug; "FETCH_DNS_RECORDS", "zone_id={zone_id}");
        let res: ApiSuccess<Vec<DnsRecord>> = req.call()?.into_json()?;
        let mut identifiers = vec![];
        for record in res.result {
            if record_names.contains(&record.name) {
                identifiers.push((record.id, record.name))
            }
        }
        finish!(tmr, "count={}", identifiers.len());
        Ok(identifiers)
    }

    async fn get_zone_identifier(&self, agent: Arc<Agent>) -> anyhow::Result<String> {
        let zone = &self.zone;
        let token = &self.token;
        let req = agent
            .get(&format!("{}/client/v4/zones", server_url()))
            .set("accept", "application/json")
            .set("authorization", &format!("bearer {token}"))
            .query("name", &self.zone);
        let tmr = stimer!(Level::Debug; "FETCH_ZONE", "zone={zone}");
        let res: ApiSuccess<Vec<Zone>> = req.call()?.into_json()?;
        let id = match res.result.first() {
            Some(zone) => zone.id.to_string(),
            None => bail!("zone not found: {zone}"),
        };
        finish!(tmr, "zone_id={id}");
        Ok(id)
    }

    /// Perform DNS record update on Cloudflare
    pub async fn run(&self) -> anyhow::Result<()> {
        let tmr = stimer!(Level::Debug; "FETCH_IP_ADDRESS");
        let current_ip = public_ip::addr_v4().await.ok_or(NoIPV4)?;
        finish!(tmr, "current_ip={current_ip:?}");

        if let Some(Cached::IP(last_ip)) = self.cache.get(&CacheKey::LastIP) {
            if current_ip == last_ip {
                debug!("IPv4 address remains unchanged, skip");
                return Ok(());
            }
            debug!("IPv4 address changed from {last_ip} to {current_ip}");
        } else {
            debug!("no previous IPv4 address found, continue");
        }

        let agent = Arc::new(self.build_agent());
        let zone_id = self.get_zone_identifier(agent.clone()).await?;
        let record_identifiers = self
            .get_record_identifiers(agent.clone(), &zone_id, &self.record_names)
            .await?;

        let mut tasks = vec![];
        for (id, name) in record_identifiers {
            let agent = agent.clone();
            let zone_id = zone_id.clone();
            tasks.push(self.update_dns_record(agent, zone_id, id, name, current_ip));
        }

        let len = tasks.len();
        let tmr = stimer!(Level::Debug; "UPDATE_DNS_RECORDS", "started={len}");
        for task in futures::future::join_all(tasks).await {
            task?;
        }
        finish!(tmr, "finished={len}");

        // save current IP address when update succeeds
        self.cache.insert(CacheKey::LastIP, Cached::IP(current_ip));

        Ok(())
    }

    async fn update_dns_record<T>(
        &self,
        agent: Arc<Agent>,
        zone_id: T,
        dns_record_id: T,
        dns_record_name: T,
        current_ip: Ipv4Addr,
    ) -> anyhow::Result<()>
    where
        T: Into<Cow<'a, str>>,
    {
        let zone_id = zone_id.into();
        let dns_record_name = dns_record_name.into();
        let dns_record_id = dns_record_id.into();
        let authorization = format!("bearer {}", &self.token);

        let url = format!(
            "{}/client/v4/zones/{zone_id}/dns_records/{dns_record_id}",
            server_url()
        );
        let req = agent.put(&url).set("authorization", &authorization);
        let tmr = stimer!(Level::Debug; "UPDATE_DNS_RECORD", "zone_id={zone_id},dns_record_id={dns_record_id}");
        let res: ApiSuccess<DnsRecord> = req
            .send_json(ureq::json!({
                "type": "A",
                "name":dns_record_name,
                "content": current_ip,
                "ttl": 1 // 1 for automatic
            }))?
            .into_json()?;
        let content = match res.result.content {
            DnsContent::A { content } => content.to_string(),
            _ => "(not an A record)".into(),
        };
        finish!(tmr, "content={content}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mockito::{mock, Matcher};
    use std::sync::Arc;

    #[tokio::test]
    async fn t_get_record_identifiers() {
        let _m = mock("GET", "/client/v4/zones/1/dns_records")
            .with_status(200)
            .with_body(r#"{"success":true,"result":[{"meta":{"auto_added":false},"locked":false,"name":"record","ttl":0,"zone_id":"1","modified_on":"1970-01-01T00:00:00Z","created_on":"1970-01-01T00:00:00Z","proxiable":false,"content":"0.0.0.0","type":"A","id":"2","proxied":false,"zone_name":"zone"}],"messages":[],"errors":[]}"#)
            .create();
        let cdu = Cdu::new("token", "zone", &["record"]);
        let agent = Arc::new(cdu.build_agent());
        let identifiers = cdu
            .get_record_identifiers(agent.clone(), "1", &["record".into()])
            .await
            .unwrap();
        assert_eq!(1, identifiers.len());

        let (record_id, record_name) = identifiers.first().unwrap();
        assert_eq!(record_id, "2");
        assert_eq!(record_name, "record");
    }

    #[tokio::test]
    async fn t_get_zone_identifier() {
        let _m = mock("GET", "/client/v4/zones")
            .match_query(Matcher::UrlEncoded("name".into(), "zone".into()))
            .with_status(200)
            .with_body(r#"{"success":true,"result":[{"id":"1","name":"zone","account":{"id":"2","name":"a"},"created_on":"1970-01-01T00:00:00Z","development_mode":0,"meta":{"custom_certificate_quota":0,"page_rule_quota":0,"phishing_detected":false,"multiple_railguns_allowed":false},"modified_on":"1970-01-01T00:00:00Z","name_servers":[],"owner":{"type":"user","email":"","id":""},"paused":false,"permissions":[],"status":"active","type":"full"}],"messages":[],"errors":[]}"#)
            .create();
        let cdu = Cdu::new("token", "zone", &["record"]);
        let agent = Arc::new(cdu.build_agent());
        let zone_identifier = cdu.get_zone_identifier(agent.clone()).await.unwrap();
        assert_eq!(zone_identifier, "1");
    }

    #[tokio::test]
    async fn t_update_dns_record() {
        let _m2 = mock("PUT", "/client/v4/zones/1/dns_records/2")
            .match_body(r#"{"content":"127.0.0.1","name":"record","ttl":1,"type":"A"}"#)
            .with_status(200)
            .with_body(r#"{"success":true,"result":{"meta":{"auto_added":false},"locked":false,"name":"record","ttl":0,"zone_id":"1","modified_on":"1970-01-01T00:00:00Z","created_on":"1970-01-01T00:00:00Z","proxiable":false,"content":"0.0.0.0","type":"A","id":"2","proxied":false,"zone_name":"zone"},"messages":[],"errors":[]}"#)
            .create();
        let cdu = Cdu::new("token", "zone", &["record"]);
        let agent = Arc::new(cdu.build_agent());
        cdu.update_dns_record(
            agent.clone(),
            "1",
            "2",
            "record",
            "127.0.0.1".parse().unwrap(),
        )
        .await
        .unwrap();
    }
}
