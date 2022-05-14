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

//! Cloudflare DNS record update

use std::borrow::Cow;
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::bail;
use cloudflare::endpoints::dns::{DnsContent, DnsRecord};
use cloudflare::endpoints::zone::Zone;
use cloudflare::framework::response::ApiSuccess;
use derivative::Derivative;
use log::{debug, Level};
use logging_timer::{finish, stimer};
use thiserror::Error;
use tokio::task::JoinHandle;
use ttl_cache::TtlCache;
use ureq::{Agent, AgentBuilder};

const HTTP_TIMEOUT: u64 = 30;

/// Recoverable errors from [`Cdu`]
#[derive(Clone, Copy, Debug, Error)]
pub enum RecoverableError {
    /// Recoverable: Failed to determine IPv4 address
    #[error("failed to determine IPv4 address")]
    IpV4,
}

#[derive(Eq, PartialEq, Hash)]
enum CacheType {
    Zone,
    Record,
}

/// Cloudflare DNS Update
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Cdu<'a> {
    token: Cow<'a, str>,
    /// DNS zone
    pub zone: Cow<'a, str>,
    record_names: Vec<String>,
    #[derivative(Debug = "ignore")]
    cache: Arc<Mutex<TtlCache<(CacheType, String), String>>>,
    /// Cache latest IP address for how many seconds
    pub cache_seconds: Option<u64>,
    /// Last IP address fetched
    pub last_ip: Option<Ipv4Addr>,
}

impl<'a> Cdu<'a> {
    /// Creates a [`Cdu`]
    pub fn new<T, U>(token: T, zone: T, record_names: &'a [U]) -> Self
    where
        T: Into<Cow<'a, str>>,
        U: Display,
    {
        let capacity = record_names.len();
        Self {
            token: token.into(),
            zone: zone.into(),
            record_names: record_names
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
            // zone identifier and record identifiers
            cache: Arc::new(Mutex::new(TtlCache::new(capacity + 1))),
            cache_seconds: None,
            last_ip: None,
        }
    }

    pub(crate) fn cache_ttl(&self) -> Option<Duration> {
        self.cache_seconds.map(Duration::from_secs)
    }

    async fn get_zone_identifier(&self, agent: Arc<Agent>) -> anyhow::Result<String> {
        if let Some(id) = self
            .cache
            .lock()
            .unwrap()
            .get(&(CacheType::Zone, self.zone.to_string()))
        {
            debug!("zone found in cache: {} ({})", &self.zone, &id);
            return Ok(id.clone());
        }

        let url = "https://api.cloudflare.com/client/v4/zones";
        let value = format!("bearer {}", self.token);
        let req = agent
            .get(&url)
            .set("accept", "application/json")
            .set("authorization", &value)
            .query("name", &self.zone);

        let tmr = stimer!(Level::Debug; "FETCH_ZONE", "zone {}", self.zone);
        let res: ApiSuccess<Vec<Zone>> = req.call()?.into_json()?;
        let id = match res.result.first() {
            Some(zone) => zone.id.to_string(),
            None => bail!("zone not found: {}", self.zone),
        };
        if let Some(ttl) = self.cache_ttl() {
            let mut cache = self.cache.lock().unwrap();
            cache.insert((CacheType::Zone, self.zone.to_string()), id.clone(), ttl);
        }
        finish!(tmr, "zone ID {}", &id);
        Ok(id)
    }

    fn build_agent(&self) -> Agent {
        AgentBuilder::new()
            .timeout(Duration::from_secs(HTTP_TIMEOUT))
            .build()
    }

    /// Perform DNS record update on Cloudflare
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let tmr = stimer!(Level::Debug; "FETCH_IP_ADDRESS");
        let current_ip = public_ip::addr_v4().await.ok_or(RecoverableError::IpV4)?;
        finish!(tmr, "public IP address {:?}", &current_ip);

        if let Some(last_ip) = self.last_ip {
            if current_ip == last_ip {
                debug!("IPv4 address remains unchanged, skip");
                return Ok(());
            }
            debug!("IPv4 address changed from {} to {}", last_ip, current_ip);
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
            let cache = self.cache.clone();
            let cache_ttl = self.cache_ttl();
            let authorization = format!("bearer {}", self.token);
            tasks.push(tokio::spawn(async move {
                if let Some(id) = cache
                    .lock()
                    .unwrap()
                    .get(&(CacheType::Record, record_name.clone()))
                {
                    debug!("record found in cache: {} ({})", &record_name, &id);
                    return Ok((id.clone(), record_name));
                }

                let url =
                    format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
                let req = agent
                    .get(&url)
                    .query("name", &record_name)
                    .set("content-type", "application/json")
                    .set("authorization", &authorization);
                let tmr = stimer!(Level::Debug; "FETCH_DNS_RECORD", "DNS record {}", &record_name);
                let res: ApiSuccess<Vec<DnsRecord>> = req.call()?.into_json()?;
                let id = match res.result.first() {
                    Some(dns_record) => dns_record.id.clone(),
                    None => bail!("DNS record not found: {}", record_name),
                };
                if let Some(ttl) = cache_ttl {
                    cache.lock().unwrap().insert(
                        (CacheType::Record, record_name.clone()),
                        id.clone(),
                        ttl,
                    );
                }
                finish!(tmr, "DNS record ID {}", &id);
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
            let authorization = format!("bearer {}", self.token);
            tasks.push(tokio::spawn(async move {
                let url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{dns_record_id}");
                let req = agent.put(&url).set("authorization", &authorization);
                let tmr = stimer!(Level::Debug; "UPDATE_DNS_RECORD", "DNS record {} ({})", &record_name, &dns_record_id);
                let res:ApiSuccess<DnsRecord> = req.send_json(ureq::json!({
                    "type":"A",
                    "name": &record_name,
                    "content": current_ip,
                    "ttl":1 // 1 for automatic
                }))?.into_json()?;
                let content = match res.result.content {
                    DnsContent::A { content } => content.to_string(),
                    _ => "(not an A record)".into(),
                };
                finish!(tmr, "content {}", &content);
                Ok(())
            }));
        }

        let tmr = stimer!(Level::Debug; "UPDATE_DNS_RECORDS", "update {} DNS records", tasks.len());
        for task in futures::future::join_all(tasks).await {
            task??;
        }
        finish!(tmr);

        // save current IP address when update succeeds
        self.last_ip = Some(current_ip);

        Ok(())
    }
}
