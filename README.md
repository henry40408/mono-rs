# mono.rs

> Mono repository for my Rust projects

![CI]( https://img.shields.io/github/actions/workflow/status/henry40408/mono-rs/workflow.yml?branch=master)
![license](https://img.shields.io/github/license/henry40408/mono-rs)
![top languages](https://img.shields.io/github/languages/top/henry40408/mono-rs)

## Projects

1. [cdu](cdu/README.md) **C**loudflare **D**NS **U**pdate
2. [hcc](hcc/README.md) **H**TTPS **C**ertificate **C**heck
4. [pushover](pushover/README.md) Pushover API wrapper with attachment support in Rust 2021 edition
5. [wfs](wfs/README.md) **W**ait **F**or **S**ignal

## Toolchain & Targets

![rust](https://img.shields.io/badge/rust-1.67.0%20|%20stable%20|%20nightly-blue)
![arch](https://img.shields.io/badge/arch-amd64%20%7C%20arm64-blue)
![os](https://img.shields.io/badge/os-linux%20%7C%20macos%20%7C%20windows-blue)
![libc](https://img.shields.io/badge/libc-gnu%20%7C%20musl%20%7C%20msvc-blue)

> wfs supports Unix platform only

## Stability

- Crates in this repository will not be published to crates.io because I don't have much time to maintain compatibility.
- [Specifying dependencies from `git` repositories](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories) is suggested.

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.

## License

[MIT](https://choosealicense.com/licenses/mit/)
