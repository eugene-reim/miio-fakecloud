# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Build stage
# ---------------------------------------------------------------------------
FROM rust:1-bookworm AS builder
WORKDIR /usr/src/app

# Cache dependencies: copy manifests first, build a dummy project, then
# replace the sources. This way dependency crates are only rebuilt when
# Cargo.toml / Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Now copy the real sources and rebuild (only the local crate is recompiled).
COPY src ./src
RUN touch src/main.rs \
    && cargo build --release \
    && strip target/release/miio-fakecloud

# ---------------------------------------------------------------------------
# Runtime stage
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# The binary does not need CA certificates (no outbound HTTPS).
# Create a dedicated non-root user for better security.
RUN useradd --system --no-create-home --uid 10001 fakecloud

COPY --from=builder /usr/src/app/target/release/miio-fakecloud /usr/local/bin/miio-fakecloud

USER 10001

EXPOSE 8053/udp
EXPOSE 8053/tcp

ENTRYPOINT ["miio-fakecloud"]
