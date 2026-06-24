FROM rust:1.85-slim AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin larder-tui --bin larder --bin larder-server
RUN cp target/release/larder-tui /usr/local/bin/
RUN cp target/release/larder /usr/local/bin/
RUN cp target/release/larder-server /usr/local/bin/

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /data /app/static
COPY --from=builder /usr/local/bin/larder-tui /usr/local/bin/
COPY --from=builder /usr/local/bin/larder /usr/local/bin/
COPY --from=builder /usr/local/bin/larder-server /usr/local/bin/
COPY --from=builder /app/server/src/static /app/static

WORKDIR /data

ENV DATABASE_URL=sqlite:/data/larder.db
ENV LARDER_STATIC_DIR=/app/static

EXPOSE 8080

VOLUME ["/data"]

ENTRYPOINT ["larder-server"]
