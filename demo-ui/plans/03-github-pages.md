# Demo UI — GitHub Pages

Deploy the static Vite build to a **project site**: `https://<user>.github.io/rust-cards-api/`.

## Goals

- Host demo-ui on GitHub Pages (Actions deploy, no server-side proxy)
- Call the public `uniques-http-api` over HTTPS with existing permissive CORS
- Keep local dev unchanged (Vite proxy, empty `VITE_API_BASE_URL`)

## Configuration

Two build-time env vars (injected in CI or local prod build):

| Variable | Purpose | GitHub Pages value |
|----------|---------|-------------------|
| `VITE_BASE_PATH` | Vite `base` for asset URLs | `/rust-cards-api/` |
| `VITE_API_BASE_URL` | API origin (scheme + host, no path) | e.g. `https://….run.app` |

`VITE_API_BASE_URL` must **not** include `/api` or a trailing slash. The UI appends `/api/v2/cards`, `/api/v2/effects`, etc.

Dev: leave both unset (or only set in `.env` for manual prod preview). `VITE_BASE_PATH` defaults to `/`.

## CI

Workflow: [`.github/workflows/deploy-demo-ui.yml`](../../.github/workflows/deploy-demo-ui.yml)

- Trigger: push to `main` when `demo-ui/**` or the workflow file changes
- Build in `demo-ui/` with `npm ci` + `npm run build`
- Deploy `demo-ui/dist` via `actions/deploy-pages`

## One-time repo setup

1. **Settings → Pages → Build and deployment → Source:** GitHub Actions
2. **Settings → Secrets and variables → Actions → Variables:** add `VITE_API_BASE_URL` with your public API URL (Cloud Run or other; see [`uniques-http-api/README.md`](../../uniques-http-api/README.md))

After the first successful workflow run, the site is at `https://<user>.github.io/rust-cards-api/`.

## Local production build

```bash
cd demo-ui
# bash / macOS / Linux
VITE_API_BASE_URL=https://your-api.example.com VITE_BASE_PATH=/rust-cards-api/ npm run build
npm run preview
```

PowerShell:

```powershell
$env:VITE_API_BASE_URL='https://your-api.example.com'
$env:VITE_BASE_PATH='/rust-cards-api/'
npm run build
npm run preview
```

Confirm network requests target `https://your-api.example.com/api/v2/...`.

## Notes

- GitHub Pages serves static files only; the dev `/api` proxy does not apply in production.
- No client router today; users should open the site root. For future SPA routes, add `public/404.html` as a copy of `index.html`.
