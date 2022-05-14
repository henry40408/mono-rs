.PHONY: build load push

REGISTRY?=registry-1.docker.io
TAG=$(shell git rev-parse --short HEAD)

build:
	cross build --release --target x86_64-unknown-linux-musl
	find target/x86_64-unknown-linux-musl -maxdepth 2 -type f -executable -exec upx --best {} \+
	cross build --release --target armv7-unknown-linux-musleabihf
	find target/armv7-unknown-linux-musleabihf -maxdepth 2 -type f -executable -exec upx --best {} \+

load: build
	docker buildx build --no-cache --load -t $(REGISTRY)/henry40408/mono:$(TAG) .

push: build
	docker buildx build --no-cache --push --platform linux/amd64,linux/arm64 -t $(REGISTRY)/henry40408/mono:$(TAG) .
