FROM scratch

COPY target/x86_64-unknown-linux-musl/release/cdu \
    target/x86_64-unknown-linux-musl/release/hcc \
    target/x86_64-unknown-linux-musl/release/hcc-pushover \
    target/x86_64-unknown-linux-musl/release/hcc-server \
    target/x86_64-unknown-linux-musl/release/po \
    target/x86_64-unknown-linux-musl/release/pop \
    target/x86_64-unknown-linux-musl/release/wfs /

CMD ["/wfs"]
