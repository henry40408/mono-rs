.PHONY: all

all: arm64 darwin linux

arm64: cdu hcc pop pushover
	RUSTFLAGS='-C link-arg=-s' cross build --release --target armv7-unknown-linux-musleabihf

darwin: cdu hcc pop pushover
	RUSTFLAGS='-C link-arg=-s' cross build --release --target x86_64-apple-darwin

linux: cdu hcc pop pushover
	RUSTFLAGS='-C link-arg=-s' cross build --release --target x86_64-unknown-linux-musl
