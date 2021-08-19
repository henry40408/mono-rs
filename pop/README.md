# pop

> **P**ush**O**ver **P**roxy

## Features

* Compatible with [Pushover API](https://pushover.net/api)
* Attach `image_url` in request body to attach image on notification

## Usage

### Server

```bash
$ export PUSHOVER_TOKEN=[Pushover API token]
$ export PUSHOVER_USER=[Pushover user key]
$ cargo run
```

### Help

```bash
cargo run -- -h
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.
