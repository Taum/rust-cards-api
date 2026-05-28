## Plan 06: Dockerize `uniques-http-api` for GCP Cloud Run

### Goal

Build and run `uniques-http-api` as a **GCP Cloud Run** service, with the merged `ALL_SETS` index **embedded inside the container image** so the service can start without external storage.

### Cloud Run requirements

- **Port binding**: Cloud Run provides an environment variable `PORT` and expects the service to bind `0.0.0.0:$PORT`.
  - Implementation: read `PORT` from env, default to `8080` when unset/blank (still Cloud Run–compatible).
- **Stateless container**: the service must not depend on a writable filesystem for startup.
  - The index is read-only and can live in the image.

### Runtime configuration

- **`PORT`**
  - Production: provided by Cloud Run as `PORT` (service binds `0.0.0.0:$PORT`).
  - Default fallback: `8080`.
  - Local dev: keep `8234` by setting `PORT=8234` in `.env.local`.
- **`INDEX_PATH`**
  - Production default (in image): `/opt/index/ALL_SETS`.
  - Local dev default: `../alt-indexer/full_index/ALL_SETS`.
  - Can be overridden via env.

### Docker image contents

The image must contain:

- The compiled `uniques-http-api` binary
- The full merged index directory:
  - Source in repo/build context: `alt-indexer/full_index/ALL_SETS`
  - Destination in image: `/opt/index/ALL_SETS`

### Docker build approach

Use a **multi-stage** Docker build:

1) **Builder stage** (`rust:*`)
   - Copy `alt-indexer/` and `uniques-http-api/` (path dependency) into the build context.
   - Build `uniques-http-api` in release mode.
2) **Runtime stage** (`debian:bookworm-slim`)
   - Install CA certs.
   - Create a non-root user and run the service as that user.
   - Set defaults:
     - `ENV INDEX_PATH=/opt/index/ALL_SETS`
     - `ENV PORT=8080`
   - `ENTRYPOINT ["/app/uniques-http-api"]`
3) **Embed the index**
   - Copy `alt-indexer/full_index/ALL_SETS` into `/opt/index/ALL_SETS`.
4) **Set container defaults**
   - Ensure `INDEX_PATH` defaults to `/opt/index/ALL_SETS` (overrideable via env).
   - Ensure the server binds `0.0.0.0:$PORT` (default `8080`).
5) **Build the image (from repo root)**

```bash
docker build -t uniques-http-api .
```

### Build context hygiene

Add `.dockerignore` to avoid sending large/unneeded files into the Docker build context, especially:

- `**/target/**`
- VCS and editor metadata (`**/.git/**`, `**/.cursor/**`)
