FROM alpine:3.12 AS builder

ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
  "linux/amd64") echo x86_64-unknown-linux-musl > /target.txt;; \
  "linux/arm64") echo armv7-unknown-linux-musleabihf > /target.txt ;; \
  *) exit 1 ;; \
esac

COPY . .

RUN cp /target/$(cat /target.txt)/release/* /tmp/

FROM scratch

COPY --from=builder /tmp/cdu /
COPY --from=builder /tmp/hcc /
COPY --from=builder /tmp/hcc-pushover /
COPY --from=builder /tmp/po /
COPY --from=builder /tmp/wfs /

CMD ["/wfs"]
