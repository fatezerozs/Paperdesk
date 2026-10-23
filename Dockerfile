# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
COPY web ./web
COPY assets ./assets
COPY examples ./examples
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked && cp target/release/paperdesk /build/paperdesk

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl poppler-utils tesseract-ocr tesseract-ocr-chi-sim \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 paperdesk && useradd --uid 10001 --gid 10001 --no-create-home paperdesk \
    && mkdir /data && chown paperdesk:paperdesk /data
COPY --from=builder /build/paperdesk /usr/local/bin/paperdesk
COPY assets/fonts/OFL.txt /usr/share/doc/paperdesk/FONT-LICENSE.txt
USER paperdesk:paperdesk
ENV PAPERDESK_BIND=0.0.0.0 PAPERDESK_PORT=3210 PAPERDESK_DATA=/data
VOLUME ["/data"]
EXPOSE 3210
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3210/api/health || exit 1
ENTRYPOINT ["/usr/local/bin/paperdesk"]
