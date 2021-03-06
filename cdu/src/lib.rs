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
use derivative::Derivative;
use log::{debug, Level};
use logging_timer::{finish, stimer};
use moka::sync::Cache;
use tokio::task::JoinHandle;
use ureq::{Agent, AgentBuilder};

const API_HOST: &str = "https://api.cloudflare.com";
const HTTP_TIMEOUT: u64 = 30;

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct NoIPV4;

impl Display for NoIPV4 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot determine public IPv4 address")
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
#[doc(hidden)]
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Cdu<'a> {
    token: Cow<'a, str>,
    zone: Cow<'a, str>,
    record_names: Vec<String>,
    #[derivative(Debug = "ignore")]
    cache: Cache<CacheKey, Cached>,
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

    async fn get_zone_identifier(&self, agent: Arc<Agent>) -> anyhow::Result<String> {
        let zone = &self.zone;
        let token = &self.token;
        let req = agent
            .get(&format!("{API_HOST}/client/v4/zones"))
            .set("accept", "application/json")
            .set("authorization", &format!("bearer {token}"))
            .query("name", &self.zone);
        let tmr = stimer!(Level::Debug; "FETCH_ZONE", "zone {zone}");
        let res: ApiSuccess<Vec<Zone>> = req.call()?.into_json()?;
        let id = match res.result.first() {
            Some(zone) => zone.id.to_string(),
            None => bail!("zone not found: {zone}"),
        };
        finish!(tmr, "zone ID {id}");
        Ok(id)
    }

    fn build_agent(&self) -> Agent {
        AgentBuilder::new()
            .timeout(Duration::from_secs(HTTP_TIMEOUT))
            .build()
    }

    /// Perform DNS record update on Cloudflare
    pub async fn run(&self) -> anyhow::Result<()> {
        let tmr = stimer!(Level::Debug; "FETCH_IP_ADDRESS");
        let current_ip = public_ip::addr_v4().await.ok_or(NoIPV4)?;
        finish!(tmr, "public IP address {current_ip:?}");

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

        let mut tasks = vec![];
        for record_name in &self.record_names {
            let agent = agent.clone();
            let record_name = record_name.clone();
            let zone_id = zone_id.clone();
            let token = &self.token;
            let authorization = format!("bearer {token}");
            tasks.push(tokio::spawn(async move {
                let url = format!("{API_HOST}/client/v4/zones/{zone_id}/dns_records");
                let req = agent
                    .get(&url)
                    .query("name", &record_name)
                    .set("content-type", "application/json")
                    .set("authorization", &authorization);
                let tmr = stimer!(Level::Debug; "FETCH_DNS_RECORD", "DNS record {record_name}");
                let res: ApiSuccess<Vec<DnsRecord>> = req.call()?.into_json()?;
                let id = match res.result.first() {
                    Some(dns_record) => dns_record.id.clone(),
                    None => bail!("DNS record not found: {record_name}"),
                };
                finish!(tmr, "DNS record ID {id}");
                Ok((id, record_name))
            }));
        }

        let mut dns_record_ids = vec![];
        for task in futures::future::join_all(tasks).await {
            let (dns_record_id, record_name) = task??;
            dns_record_ids.push((dns_record_id, record_name));
        }

        let mut tasks: Vec<JoinHandle<anyhow::Result<()>>> = vec![];
        for (dns_record_id, record_name) in dns_record_ids {
            let agent = agent.clone();
            let zone_id = zone_id.clone();
            let token = &self.token;
            let authorization = format!("bearer {token}");
            tasks.push(tokio::spawn(async move {
                let url = format!("{API_HOST}/client/v4/zones/{zone_id}/dns_records/{dns_record_id}");
                let req = agent.put(&url).set("authorization", &authorization);
                let tmr = stimer!(Level::Debug; "UPDATE_DNS_RECORD", "DNS record {record_name} ({dns_record_id})");
                let res:ApiSuccess<DnsRecord> = req.send_json(ureq::json!({
                    "type": "A",
                    "name": &record_name,
                    "content": current_ip,
                    "ttl": 1 // 1 for automatic
                }))?.into_json()?;
                let content = match res.result.content {
                    DnsContent::A { content } => content.to_string(),
                    _ => "(not an A record)".into(),
                };
                finish!(tmr, "content {}", &content);
                Ok(())
            }));
        }

        let len = tasks.len();
        let tmr = stimer!(Level::Debug; "UPDATE_DNS_RECORDS", "update {len} DNS records");
        for task in futures::future::join_all(tasks).await {
            task??;
        }
        finish!(tmr);

        // save current IP address when update succeeds
        self.cache.insert(CacheKey::LastIP, Cached::IP(current_ip));

        Ok(())
    }
}
