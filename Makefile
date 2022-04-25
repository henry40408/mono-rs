.PHONY: all push

REGISTRY?=registry-1.docker.io
TAG=$(shell git rev-parse --short HEAD)

all: push

push:
	cross build --release --target x86_64-unknown-linux-musl
	cross build --release --target armv7-unknown-linux-musleabihf
	docker buildx build --no-cache --push --platform linux/amd64,linux/arm64 -t $(REGISTRY)/henry40408/mono:$(TAG) .
