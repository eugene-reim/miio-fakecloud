# miio-fakecloud

A tiny, dependency-free Rust server that emulates the Xiaomi miIO cloud endpoints
(`ott.io.mi.com`, `ot.io.mi.com`). It replies to device handshakes and keep-alives so
that miIO hardware keeps working even when it cannot reach the real cloud.

- 100% safe Rust, standard library only, no external dependencies
- No configuration, no command line arguments
- Listens on UDP and TCP port `8053`

## Build

```
cargo build --release
```

The binary is placed at `target/release/miio-fakecloud`.

## Run

```
./target/release/miio-fakecloud
```

Point your devices at this server by configuring DNS overrides for `*.io.mi.com` (e.g.
`203.0.113.1`) and Destination NAT rules:

- UDP `203.0.113.1:8053` → `<this-host>:8053`
- TCP `203.0.113.1:80` → `<this-host>:8053`

## Docker

A multi-arch image (`linux/amd64`, `linux/arm64`) is published to GitHub Container
Registry:

```
docker run --rm -p 8053:8053/udp -p 8053:8053 ghcr.io/eugene-reim/miio-fakecloud:latest
```

Or with the provided Compose file:

```
docker compose up -d
```