# Stage 1: Build Web-UI
FROM node:22.14.0-alpine3.21 AS ui-builder
WORKDIR /app/web-ui
COPY web-ui/ .
RUN npm ci && npm run build

# Stage 2: Build Rust
FROM rust:1.85.1-bookworm AS rust-builder
RUN apt-get update && apt-get install -y protobuf-compiler libclang-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY --from=ui-builder /app/web-ui/dist web-ui/dist
RUN cargo build --release --bin amigo-server --bin amigo-dl

# Stage 3: Runtime
FROM debian:bookworm-20250317-slim

# Install runtime dependencies (ca-certificates for TLS, wget for healthcheck)
RUN apt-get update && apt-get install -y ca-certificates wget && rm -rf /var/lib/apt/lists/*

# Create non-root user (UID 1000)
RUN groupadd -r amigo && useradd -r -g amigo -d /config -s /sbin/nologin amigo

COPY --from=rust-builder /app/target/release/amigo-server /usr/local/bin/
COPY --from=rust-builder /app/target/release/amigo-dl /usr/local/bin/
COPY plugins/ /etc/amigo/plugins/
COPY locales/ /etc/amigo/locales/

# Create directories and set ownership
RUN mkdir -p /config /downloads && \
    chown -R amigo:amigo /config /downloads /etc/amigo/plugins /etc/amigo/locales

EXPOSE 1516
VOLUME ["/config", "/downloads"]

ENV AMIGO_CONFIG_DIR=/config
ENV AMIGO_DOWNLOAD_DIR=/downloads
ENV AMIGO_PLUGIN_DIR=/etc/amigo/plugins
ENV AMIGO_LOCALES_DIR=/etc/amigo/locales

# Health check against the status endpoint
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=10s \
    CMD wget --no-verbose --tries=1 --spider http://localhost:1516/api/v1/status || exit 1

# Run as non-root user (UID 1000)
# Note: Host-mounted volumes (/config, /downloads) must be writable by UID 1000
USER amigo

ENTRYPOINT ["amigo-server"]
