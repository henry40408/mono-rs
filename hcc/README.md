# hcc

> **H**TTPS **C**ertificate **C**heck

## Features

* A daemon checks HTTPS certificates periodically with cron
* An HTTP server performs checks domain names on demand
* Daemon can send check results to [Pushover](https://pushover.net/)

## Usage

### CLI

```bash
hcc check httpbin.org
```

### Server

```bash
hcc-server
```

Usage:

```bash
$ curl :9292/sha512.badssl.com
{"state":"OK","checked_at":"2021-06-01T07:45:24+00:00","days":304,"domain_name":"sha512.badssl.com","expired_at":"2022-04-01T12:00:00+00:00","elapsed":364}

$ curl :9292/expired.badssl.com
{"state":"EXPIPRED","checked_at":"2021-06-01T07:45:24+00:00","days":0,"domain_name":"expired.badssl.com","expired_at":"1970-01-01T00:00:00+00:00","elapsed":0}

$ curl :9292/sha512.badssl.com,expired.badssl.com
[{"state":"OK","checked_at":"2021-06-01T07:45:24+00:00","days":304,"domain_name":"sha512.badssl.com","expired_at":"2022-04-01T12:00:00+00:00","elapsed":172},{"state":"EXPIPRED","checked_at":"2021-06-01T07:45:24+00:00","days":0,"domain_name":"expired.badssl.com","expired_at":"1970-01-01T00:00:00+00:00","elapsed":0}]
```

### Daemon and Pushover

```bash
$ export DOMAIN_NAMES=www.example.com,sha512.badssl.com
$ export PUSHOVER_TOKEN=[Pushover API token]
$ export PUSHOVER_USER=[Pushover user key]
$ hcc-pushover
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.
