## Multi-stage build for Cloud Run.
##
## Build context must include:
## - uniques-http-api/
## - alt-indexer/
## - alt-indexer/full_index/ALL_SETS (embedded into the image)
##
## Example:
##   docker build -t uniques-http-api .

FROM rust:1.86-bookworm AS builder

WORKDIR /app

# Copy only the crates needed to build the binary (alt-indexer is a path dependency).
COPY alt-indexer/ ./alt-indexer/
COPY uniques-http-api/ ./uniques-http-api/

RUN cargo build --release --manifest-path uniques-http-api/Cargo.toml


FROM debian:bookworm-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin app

ENV INDEX_PATH=/opt/index/ALL_SETS
ENV PORT=8080

WORKDIR /app

COPY --from=builder /app/uniques-http-api/target/release/uniques-http-api /app/uniques-http-api
COPY alt-indexer/full_index/ALL_SETS /opt/index/ALL_SETS

RUN chown -R app:app /app /opt/index

USER app

EXPOSE 8080

ENTRYPOINT ["/app/uniques-http-api"]

