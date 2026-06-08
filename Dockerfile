# Multi-stage build producing a slim image with just the `dbsnap` binary.
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --locked -p dbsnap-cli

FROM debian:bookworm-slim
# native-tls links the system OpenSSL; we also need CA certificates for TLS.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dbsnap /usr/local/bin/dbsnap
ENTRYPOINT ["dbsnap"]
CMD ["--help"]
