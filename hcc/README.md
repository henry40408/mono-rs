# hcc

> **H**TTPS **C**ertificate **C**heck

## Features

* A daemon checks HTTPS certificates periodically with cron
* Daemon can send check results to [Pushover](https://pushover.net/)

## Usage

### CLI

```bash
hcc check httpbin.org
```

### Daemon and Pushover

```bash
$ export DOMAIN_NAMES=www.example.com,sha256.badssl.com
$ export PUSHOVER_TOKEN=[Pushover API token]
$ export PUSHOVER_USER=[Pushover user key]
$ hcc daemon
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.
