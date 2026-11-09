# syntax=docker/dockerfile:1.7
# Larder: Rust API server + Flutter web UI in one image.

ARG FLUTTER_VERSION=3.47.9

# ---------- Flutter web build ----------
FROM debian:bookworm-slim AS web
ARG FLUTTER_VERSION
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl git unzip xz-utils \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" \
      | tar -xJ -C /opt \
    && git config --global --add safe.directory /opt/flutter
ENV PATH="/opt/flutter/bin:${PATH}"
RUN flutter --disable-analytics && flutter config --no-cli-animations && flutter precache --web
WORKDIR /src/app
COPY app/pubspec.yaml app/pubspec.lock* ./
RUN flutter pub get
COPY app/ ./
RUN flutter build web --release --no-web-resources-cdn

# ---------- Rust server build ----------
FROM rust:1-bookworm AS server
WORKDIR /src/server
COPY server/ ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/server/target \
    cargo build --release --locked -p larder-server --bin larder \
    && cp target/release/larder /usr/local/bin/larder

# ---------- Runtime ----------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data --shell /usr/sbin/nologin larder \
    && mkdir -p /data && chown larder /data
COPY --from=server /usr/local/bin/larder /usr/local/bin/larder
COPY --from=web /src/app/build/web /app/web
ENV LARDER_BIND=0.0.0.0:8080 \
    LARDER_DATA_DIR=/data \
    LARDER_WEB_DIR=/app/web
USER larder
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD ["larder", "healthcheck"]
ENTRYPOINT ["larder"]
