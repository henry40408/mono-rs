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
use cloudflare::endpoints::dns::{
    DnsContent, DnsRecord, ListDnsRecords, ListDnsRecordsParams, UpdateDnsRecord,
    UpdateDnsRecordParams,
};
use cloudflare::endpoints::zone::{ListZones, ListZonesParams, Zone};
use cloudflare::framework::async_api::{ApiClient, Client};
use cloudflare::framework::auth::Credentials;
use cloudflare::framework::response::ApiSuccess;
use cloudflare::framework::{Environment, HttpApiClientConfig};
use derivative::Derivative;
use log::{debug, Level};
use logging_timer::{finish, stimer};
use thiserror::Error;
use tokio::task::JoinHandle;
use ttl_cache::TtlCache;

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

    async fn get_zone_identifier(&self, client: Arc<Client>) -> anyhow::Result<String> {
        if let Some(id) = self
            .cache
            .lock()
            .unwrap()
            .get(&(CacheType::Zone, self.zone.to_string()))
        {
            debug!("zone found in cache: {} ({})", &self.zone, &id);
            return Ok(id.clone());
        }

        let params = ListZones {
            params: ListZonesParams {
                name: Some(self.zone.to_string()),
                ..Default::default()
            },
        };

        let tmr = stimer!(Level::Debug; "FETCH_ZONE", "zone {}", self.zone);
        let res: ApiSuccess<Vec<Zone>> = client.request(&params).await?;
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

    fn build_client(&self) -> anyhow::Result<Client> {
        let credentials = Credentials::UserAuthToken {
            token: self.token.to_string(),
        };
        let config = HttpApiClientConfig {
            http_timeout: Duration::from_secs(HTTP_TIMEOUT),
            ..Default::default()
        };
        Client::new(credentials, config, Environment::Production)
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

        let client = Arc::new(self.build_client()?);
        let zone_id = self.get_zone_identifier(client.clone()).await?;

        let mut tasks = vec![];
        for record_name in &self.record_names {
            let client = client.clone();
            let record_name = record_name.clone();
            let zone_id = zone_id.clone();
            let cache = self.cache.clone();
            let cache_ttl = self.cache_ttl();
            tasks.push(tokio::spawn(async move {
                if let Some(id) = cache
                    .lock()
                    .unwrap()
                    .get(&(CacheType::Record, record_name.clone()))
                {
                    debug!("record found in cache: {} ({})", &record_name, &id);
                    return Ok((id.clone(), record_name));
                }
                let params = ListDnsRecords {
                    zone_identifier: &zone_id,
                    params: ListDnsRecordsParams {
                        name: Some(record_name.clone()),
                        ..Default::default()
                    },
                };
                let tmr = stimer!(Level::Debug; "FETCH_DNS_RECORD", "DNS record {}", &record_name);
                let res: ApiSuccess<Vec<DnsRecord>> = client.request(&params).await?;
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
            let client = client.clone();
            let zone_id = zone_id.clone();
            tasks.push(tokio::spawn(async move {
                let params = UpdateDnsRecord {
                    zone_identifier: &zone_id,
                    identifier: &dns_record_id,
                    params: UpdateDnsRecordParams {
                        name: &record_name,
                        content: DnsContent::A {
                            content: current_ip,
                        },
                        proxied: None,
                        ttl: None,
                    },
                };
                let tmr = stimer!(Level::Debug; "UPDATE_DNS_RECORD", "DNS record {} ({})", &record_name, &dns_record_id);
                let res: ApiSuccess<DnsRecord> = client.request(&params).await?;
                let dns_record = res.result;
                let content = match dns_record.content {
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
