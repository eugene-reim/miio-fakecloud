# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/miio-fakecloud /usr/local/bin/miio-fakecloud

EXPOSE 8053/udp
EXPOSE 8053/tcp

ENTRYPOINT ["miio-fakecloud"]
