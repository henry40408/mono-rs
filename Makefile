.PHONY: all clean amd64 arm64 push manifest

DIGEST=$(shell git rev-parse --short HEAD)
REGISTRY?=registry-1.docker.io

all: manifest

amd64:
	RUSTFLAGS="-C link-arg=-s" cross build --release --target x86_64-unknown-linux-musl
	DOCKER_BUILDKIT=1 docker build -t ${REGISTRY}/henry40408/mono:${DIGEST}-amd64 .

arm64:
	RUSTFLAGS="-C link-arg=-s" cross build --release --target armv7-unknown-linux-musleabihf
	DOCKER_BUILDKIT=1 docker build -t ${REGISTRY}/henry40408/mono:${DIGEST}-arm64 -f Dockerfile.arm64 .

clean:
	cargo clean

push: amd64 arm64
	docker push ${REGISTRY}/henry40408/mono:${DIGEST}-amd64
	docker push ${REGISTRY}/henry40408/mono:${DIGEST}-arm64

manifest: push
	docker manifest create --amend ${REGISTRY}/henry40408/mono:${DIGEST} ${REGISTRY}/henry40408/mono:${DIGEST}-amd64 ${REGISTRY}/henry40408/mono:${DIGEST}-arm64
	docker manifest annotate ${REGISTRY}/henry40408/mono:${DIGEST} ${REGISTRY}/henry40408/mono:${DIGEST}-arm64 --os linux --arch arm64
	docker manifest annotate ${REGISTRY}/henry40408/mono:${DIGEST} ${REGISTRY}/henry40408/mono:${DIGEST}-amd64 --os linux --arch amd64
	docker manifest push ${REGISTRY}/henry40408/mono:${DIGEST}
