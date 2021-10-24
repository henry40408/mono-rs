use std::cell::RefCell;
use std::fmt::Formatter;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
use log::{debug, info};
use tokio::task::JoinHandle;
use ttl_cache::TtlCache;

use crate::{Opts, PublicIPError};

const HTTP_TIMEOUT: u64 = 30;

const ZONE: u8 = 1;
const RECORD: u8 = 2;

/// Cloudflare DNS Update
pub struct Cdu<'a> {
    opts: &'a Opts,
    cache: Arc<Mutex<TtlCache<(u8, String), String>>>,
    last_ip: RefCell<Option<Ipv4Addr>>,
}

impl<'a> std::fmt::Debug for Cdu<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cdu {{ opts: {:?} }}", self.opts)
    }
}

impl<'a> Cdu<'a> {
    /// Creates an [`Cdu`]
    pub fn new(opts: &'a Opts) -> Self {
        let capacity = opts.record_name_list().len();
        Self {
            opts,
            // zone identifier and record identifiers
            cache: Arc::new(Mutex::new(TtlCache::new(capacity + 1))),
            last_ip: RefCell::new(None),
        }
    }

    pub(crate) fn cache_ttl(&self) -> Option<Duration> {
        if self.opts.cache_seconds > 0 {
            Some(Duration::from_secs(self.opts.cache_seconds))
        } else {
            None
        }
    }

    /// Cron syntax from argument parser
    pub fn cron(&self) -> &str {
        &self.opts.cron
    }

    /// Is daemon mode enabled?
    pub fn is_daemon(&self) -> bool {
        self.opts.daemon
    }

    async fn get_zone_identifier(&self, client: Arc<Client>) -> anyhow::Result<(Duration, String)> {
        if let Some(id) = self
            .cache
            .lock()
            .unwrap()
            .get(&(ZONE, self.opts.zone.clone()))
        {
            debug!("zone found in cache: {} ({})", &self.opts.zone, &id);
            return Ok((Duration::from_millis(0), id.clone()));
        }

        let params = ListZones {
            params: ListZonesParams {
                name: Some(self.opts.zone.clone()),
                ..Default::default()
            },
        };

        let instant = Instant::now();
        let res: ApiSuccess<Vec<Zone>> = client.request(&params).await?;
        let duration = Instant::now() - instant;
        debug!("took {}ms to fetch zone identifier", duration.as_millis());

        let id = match res.result.first() {
            Some(zone) => zone.id.to_string(),
            None => bail!("zone not found: {}", self.opts.zone),
        };
        if let Some(ttl) = self.cache_ttl() {
            let mut cache = self.cache.lock().unwrap();
            cache.insert((ZONE, self.opts.zone.clone()), id.clone(), ttl);
        }
        debug!(
            "zone fetched from Cloudflare: {} ({})",
            &self.opts.zone, &id
        );
        Ok((duration, id))
    }

    fn build_client(&self) -> anyhow::Result<Client> {
        let credentials = Credentials::UserAuthToken {
            token: self.opts.token.clone(),
        };
        let config = HttpApiClientConfig {
            http_timeout: Duration::from_secs(HTTP_TIMEOUT),
            ..Default::default()
        };
        Client::new(credentials, config, Environment::Production)
    }

    /// Perform DNS record update on Cloudflare
    pub async fn run(&self) -> anyhow::Result<()> {
        let current_ip = public_ip::addr_v4().await.ok_or(PublicIPError)?;
        debug!("public IPv4 address: {}", &current_ip);

        let last_ip = *self.last_ip.borrow();
        if let Some(last_ip) = last_ip {
            debug!(
                "previous IPv4 address {}, current IPv4 address {}",
                last_ip, current_ip
            );
            if current_ip == last_ip {
                info!("IPv4 address remains unchanged, skip");
                return Ok(());
            }
        } else {
            debug!("current IPv4 address {}", current_ip);
        }
        *self.last_ip.borrow_mut() = Some(current_ip);

        let client = Arc::new(self.build_client()?);
        let (duration1, zone_id) = self.get_zone_identifier(client.clone()).await?;

        let mut tasks = vec![];
        for record_name in self.opts.record_name_list() {
            let client = client.clone();
            let zone_id = zone_id.clone();
            let cache = self.cache.clone();
            let cache_ttl = self.cache_ttl();
            tasks.push(tokio::spawn(async move {
                if let Some(id) = cache.lock().unwrap().get(&(RECORD, record_name.clone())) {
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
                let res: ApiSuccess<Vec<DnsRecord>> = client.request(&params).await?;
                let id = match res.result.first() {
                    Some(dns_record) => dns_record.id.clone(),
                    None => bail!("DNS record not found: {}", record_name),
                };
                if let Some(ttl) = cache_ttl {
                    cache
                        .lock()
                        .unwrap()
                        .insert((RECORD, record_name.clone()), id.clone(), ttl);
                }
                debug!("record fetched from Cloudflare: {} ({})", &record_name, &id);
                Ok((id, record_name))
            }));
        }

        let mut dns_record_ids = vec![];
        let instant = Instant::now();
        for task in futures::future::join_all(tasks).await {
            let (dns_record_id, record_name) = task??;
            dns_record_ids.push((dns_record_id, record_name));
        }
        let duration2 = Instant::now() - instant;
        debug!(
            "took {}ms to fetch record identifiers",
            duration2.as_millis()
        );

        let mut tasks: Vec<JoinHandle<anyhow::Result<(String, String, String)>>> = vec![];
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
                let res: ApiSuccess<DnsRecord> = client.request(&params).await?;
                let dns_record = res.result;
                let content = match dns_record.content {
                    DnsContent::A { content } => content.to_string(),
                    _ => "(not an A record)".into(),
                };

                Ok((record_name, dns_record_id, content))
            }));
        }

        let instant = Instant::now();
        for task in futures::future::join_all(tasks).await {
            let (r, d, c) = task??;
            debug!("DNS record updated: {} ({}) -> {}", &r, &d, &c);
        }
        let duration3 = Instant::now() - instant;
        debug!("took {}ms to update DNS records", duration3.as_millis());

        info!("took {}ms to fetch zone record, {}ms to fetch DNS records, and {}ms to update DNS records", duration1.as_millis(),
        duration2.as_millis(),duration3.as_millis());

        Ok(())
    }
}
