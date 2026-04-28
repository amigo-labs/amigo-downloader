# Stage 1: Build Web-UI (pnpm workspace)
FROM node:22-alpine3.21 AS ui-builder
RUN corepack enable
WORKDIR /app

# Copy workspace manifests first for maximum layer cache reuse
COPY pnpm-workspace.yaml pnpm-lock.yaml package.json ./
COPY web-ui/package.json web-ui/
COPY packages/ui/package.json packages/ui/

# Install only the workspace dependencies needed to build the web-ui
RUN --mount=type=cache,id=pnpm,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --filter amigo-web-ui...

# Now copy sources and build
COPY packages/ui packages/ui
COPY web-ui web-ui
RUN pnpm --filter amigo-web-ui build

# Stage 2: Build Rust
# Pinning to a minor + dated tag stops a silent base-image bump from
# changing the toolchain mid-release. Bump deliberately when the project
# moves to a new MSRV.
FROM rust:1.94-bookworm AS rust-builder
RUN apt-get update && apt-get install -y protobuf-compiler libclang-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY tauri/ tauri/
COPY --from=ui-builder /app/web-ui/dist web-ui/dist

# Plugin-registry pubkey is baked into the binary at compile time
# (crates/plugin-runtime/build.rs refuses to compile a release build
# without it, so an all-zero placeholder can never ship). Production
# images must be built with `--build-arg AMIGO_REGISTRY_PUBKEY_HEX=<real key>`.
ARG AMIGO_REGISTRY_PUBKEY_HEX
ENV AMIGO_REGISTRY_PUBKEY_HEX=${AMIGO_REGISTRY_PUBKEY_HEX}
RUN cargo build --release --bin amigo-server --bin amigo-dl

# Stage 3: Runtime
# Stays on the floating bookworm-slim tag so security updates land
# without manual intervention; rebuilds in CI are reproducible because
# the rust-builder stage above is pinned and the runtime stage only
# adds ca-certificates + wget on top. If reproducibility against a
# specific debian point release matters more than auto-patching, swap
# the tag for a dated one (e.g. debian:bookworm-20250908-slim).
FROM debian:bookworm-slim

# Install runtime dependencies (ca-certificates for TLS, wget for healthcheck)
RUN apt-get update && apt-get install -y ca-certificates wget && rm -rf /var/lib/apt/lists/*

# Create non-root user with explicit UID/GID 1000 for predictable volume permissions
RUN groupadd -r -g 1000 amigo && useradd -r -u 1000 -g amigo -d /config -s /sbin/nologin amigo

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
