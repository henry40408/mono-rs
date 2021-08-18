# hcc

![GitHub Workflow](https://github.com/henry40408/hcc/actions/workflows/workflow.yml/badge.svg) ![GitHub](https://img.shields.io/github/license/henry40408/hcc)

**H**TTPS **C**ertificate **C**heck

## Features

* A daemon checks HTTPS certificates periodically with cron
* An HTTP server performs checks with incoming requests
* Check results can be sent to [Pushover](https://pushover.net/)

## Usage

### CLI

```bash
$ cargo run --bin hcc -- check httpbin.org
```

### Server

Run as Docker container:

```bash
$ make amd64
$ docker run -it -p 9292:9292 henry40408/hcc:$(git rev-parse --short HEAD)-amd64 /hcc-server -b 0.0.0.0:9292
```

Run directly:

```bash
$ cargo run --bin hcc-server
```

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
$ DOMAIN_NAMES=www.example.com,sha512.badssl.com \
  PUSHOVER_TOKEN=token \
  PUSHOVER_USER=user \
  cargo run --bin hcc-pushover
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.

## License

[MIT](https://choosealicense.com/licenses/mit/)
