# cdu

> **C**loudflare **D**NS record **U**pdate

## Features

* A CLI to update DNS records once
* A daemon to update DNS records on Cloudflare with cron
* Cache zone and DNS record identifier for designated time span

## Usage

### CLI

```bash
$ export CLOUDFLARE_TOKEN=[your Cloudflare token]
$ export CLOUDFLARE_ZONE=[name of your zone on Cloudflare]
$ export CLOUDFLARE_RECORDS=[name of DNS records on Cloudflare, separated by comma]
$ cargo run
```

### Daemon

```bash
$ export CLOUDFLARE_TOKEN=[your Cloudflare token]
$ CLOUDFLARE_ZONE=[name of your zone on Cloudflare]
$ CLOUDFLARE_RECORDS=[name of DNS records on Cloudflare, separated by comma]
$ cargo run -- --daemon true
```

### Help

```bash
cargo run -- -h
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.
