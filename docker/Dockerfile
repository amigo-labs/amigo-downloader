# Stage 1: Build Web-UI
FROM node:22-alpine AS ui-builder
WORKDIR /app/web-ui
COPY web-ui/ .
RUN npm ci && npm run build

# Stage 2: Build Rust
FROM rust:latest AS rust-builder
RUN apt-get update && apt-get install -y protobuf-compiler libclang-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY --from=ui-builder /app/web-ui/dist crates/server/static
RUN cargo build --release --bin amigo-server --bin amigo-dl

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /app/target/release/amigo-server /usr/local/bin/
COPY --from=rust-builder /app/target/release/amigo-dl /usr/local/bin/
COPY plugins/ /etc/amigo/plugins/
COPY locales/ /etc/amigo/locales/

EXPOSE 1516
VOLUME ["/config", "/downloads"]

ENV AMIGO_CONFIG_DIR=/config
ENV AMIGO_DOWNLOAD_DIR=/downloads
ENV AMIGO_PLUGIN_DIR=/etc/amigo/plugins
ENV AMIGO_LOCALES_DIR=/etc/amigo/locales

ENTRYPOINT ["amigo-server"]
