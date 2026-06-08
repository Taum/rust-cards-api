## Multi-stage build for Cloud Run.
##
## Example:
##   docker build -t uniques-http-api .

FROM rust:1.86-bookworm AS builder

WORKDIR /app

# Copy workspace manifest and crates (index-core is a path dependency).
COPY Cargo.toml Cargo.lock ./
COPY index-core/ ./index-core/
COPY cli-indexer/ ./cli-indexer/
COPY uniques-http-api/ ./uniques-http-api/

RUN cargo build --release -p uniques-http-api

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin app

ENV APP_ENV=production
ENV CONFIG_DIR=/app/config
ENV PORT=8080

WORKDIR /app

COPY --from=builder /app/target/release/uniques-http-api /app/uniques-http-api
COPY uniques-http-api/config/default.toml /app/config/default.toml
COPY deployment/production.toml /app/config/production.toml

RUN chown -R app:app /app

USER app

EXPOSE 8080

ENTRYPOINT ["/app/uniques-http-api"]
