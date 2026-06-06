# Cards API demo UI

Simple React demo for [`GET /api/v2/cards`](../docs/api-spec.md).

See [plans/01-initial-version.md](plans/01-initial-version.md) for scope and filter syntax. See [plans/03-github-pages.md](plans/03-github-pages.md) for GitHub Pages deployment.

Demo UI is Published at `https://taum.github.io/rust-cards-api/`.

## Run

**Terminal 1 — API** (from repo root):

```bash
cargo run -p uniques-http-api
```

Uses `INDEX_PATH=../build/full_index/ALL_SETS` and port **8234** by default (see `uniques-http-api/.env.local`).

**Terminal 2 — Demo UI**:

```bash
cd demo-ui
npm install
npm run dev
```

Open the URL Vite prints (usually http://localhost:5173). Requests go to `/api/...` and are proxied to the API.

## Configuration

Copy `.env.example` to `.env` and set `VITE_API_BASE_URL` if you are not using the dev proxy (e.g. production static hosting with CORS on the API).

For GitHub Pages project-site builds, also set `VITE_BASE_PATH=/rust-cards-api/` (the deploy workflow sets this automatically).

## Build

```bash
npm run build
npm run preview
```

Production-like local build (matches GitHub Pages asset paths and cross-origin API):

```bash
VITE_API_BASE_URL=https://your-api.example.com VITE_BASE_PATH=/rust-cards-api/ npm run build
npm run preview
```