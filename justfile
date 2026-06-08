# Task runner for rust-cards-api (https://github.com/casey/just)
#
# List recipes:  just
# Recipe help:    just --list
#

set dotenv-load := true
set dotenv-path := ".env.local"

# Just uses `sh` by default, but on windows we want to use PowerShell as it is more common and easier to use.
# We will need to write custom recipes for Windows if going beyond simple passthrough commands.
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default:
    @just --list


# Build and run the HTTP API (release; loads uniques-http-api/.env.local).
[group('1-run')]
api:
    cargo run -p uniques-http-api --release

# Run the demo UI dev server (Vite; expects API at uniques-http-api/.env.local).
[group('1-run'), unix]
demo-ui:
    cd demo-ui && npm run dev
# Run the demo UI dev server (Vite; expects API at uniques-http-api/.env.local).
[group('1-run'), windows]
demo-ui:
    Set-Location demo-ui; npm run dev

# Passthrough to cli-indexer (release). Example: just cli build --help
[group('1-run')]
cli *ARGS:
    cargo run -p cli-indexer --release -- {{ARGS}}

# Passthrough to cli-indexer (debug, faster iteration). Example: just cli-dev build --help
[group('1-run')]
cli-dev *ARGS:
    cargo run -p cli-indexer -- {{ARGS}}


# Fast compile check for the whole workspace.
[group('2-dev')]
check:
    cargo check --workspace

# Run clippy (linter for code quality)
[group('2-dev')]
clippy:
    cargo clippy --workspace

# Run all workspace tests.
[group('2-dev')]
test:
    cargo test --workspace

# Build release binaries for every workspace crate.
[group('2-dev')]
build:
    cargo build --release --workspace

# Quick idGd query smoke test against the merged index.
[group('3-manual-test')]
query id="24":
    cargo run -p cli-indexer --release -- query --index-dir build/full_index --set ALL_SETS --list 3 --show-effect --id-gd {{id}}

# Quick query by reference ID
[group('3-manual-test')]
get-refid id="ALT_COREKS_B_AX_04_U_10":
    cargo run -p cli-indexer --release -- query --index-dir build/full_index --set ALL_SETS --refid {{id}}

# Build the index for a single set
[group('4-production')]
create-index set="COREKS" root_dir="../equinox-cards":
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-{{set}} --set {{set}} --out build/sets_index/{{set}}

# Build the index for all sets
[group('4-production')]
create-index-all root_dir="../equinox-cards":
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-COREKS --set COREKS --out build/sets_index/COREKS
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-CORE --set CORE --out build/sets_index/CORE
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-ALIZE --set ALIZE --out build/sets_index/ALIZE
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-BISE --set BISE --out build/sets_index/BISE
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-CYCLONE --set CYCLONE --out build/sets_index/CYCLONE
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-DUSTER --set DUSTER --out build/sets_index/DUSTER
    cargo run -p cli-indexer --release -- build --root {{root_dir}}/cards-unique-EOLE --set EOLE --out build/sets_index/EOLE
    cargo run -p cli-indexer --release -- merge --index-dir build/sets_index --sets COREKS,CORE,ALIZE,BISE,CYCLONE,DUSTER,EOLE --out build/full_index/ALL_SETS

# Merge per-set indexes under build/sets_index/ into build/full_index/ALL_SETS.
[group('4-production')]
index-merge sets="COREKS,CORE,ALIZE,BISE,CYCLONE,DUSTER,EOLE":
    cargo run -p cli-indexer --release -- merge --index-dir build/sets_index --sets {{sets}} --out build/full_index/ALL_SETS

# Compress the full index into a single .tar.zst file.
[group('4-production'), unix]
compress-index:
    tar -C build/full_index/ALL_SETS -I "zstd -19" --transform 's,^\./,,' -cvf build/full_index.tar.zst .

# Publish merged index to GCS (requires gcloud ADC + GCS_INDEX_BUCKET in .env.local).
[group('4-production'), unix]
publish-index prefix="index":
    #!/usr/bin/env bash
    set -euo pipefail
    : "${GCS_INDEX_BUCKET:?Set GCS_INDEX_BUCKET in .env.local}"
    if [[ ! -f build/full_index.tar.zst ]]; then
      echo "build/full_index.tar.zst not found; run: just compress-index" >&2
      exit 1
    fi
    archive_object="{{prefix}}/full_index.tar.zst"
    gsutil cp build/full_index.tar.zst "gs://${GCS_INDEX_BUCKET}/${archive_object}"
    version="$(python -c "import json; m=json.load(open('build/full_index/ALL_SETS/manifest.json')); print(json.dumps({'version': m['built_at_secs'], 'archive_object': '${archive_object}'}))")"
    printf '%s\n' "$version" > build/version.json
    gsutil cp build/version.json "gs://${GCS_INDEX_BUCKET}/{{prefix}}/version.json"

# Publish merged index to GCS (requires gcloud ADC + GCS_INDEX_BUCKET in .env.local).
[group('4-production'), windows]
publish-index prefix="index":
    if (-not $env:GCS_INDEX_BUCKET) { throw "Set GCS_INDEX_BUCKET in .env.local" }
    if (-not (Test-Path build/full_index.tar.zst)) { throw "build/full_index.tar.zst not found; run: just compress-index" }
    gsutil cp build/full_index.tar.zst "gs://$($env:GCS_INDEX_BUCKET)/{{prefix}}/full_index.tar.zst"
    python -c "import json; ao='{{prefix}}/full_index.tar.zst'; m=json.load(open('build/full_index/ALL_SETS/manifest.json')); json.dump({'version': m['built_at_secs'], 'archive_object': ao}, open('build/version.json','w'))"
    gsutil cp build/version.json "gs://$($env:GCS_INDEX_BUCKET)/{{prefix}}/version.json"

# Build the Cloud Run Docker image (requires build/full_index/ALL_SETS on disk).
[group('4-production')]
docker:
    docker build -t uniques-http-api .

# Build and push the Cloud Run image to Artifact Registry. Example: just docker-push 0.0.8
[group('4-production'), windows]
docker-push version:
    docker build -t $($env:DOCKER_REGISTRY):v{{version}} -f Dockerfile .
    docker push $($env:DOCKER_REGISTRY):v{{version}}

# Passthrough to image-sampler (release). Example: just img analyze --help
[group('9-img-sampler')]
img *ARGS:
    cargo run -p image-sampler --release -- {{ARGS}}

# Count ability combinations from cards.bin (no JSON crawl).
[group('9-img-sampler')]
img-analyze index_dir="build/full_index" set="ALL_SETS" out_json="out/analyze.json":
    cargo run -p image-sampler --release -- analyze --index-dir {{index_dir}} --set {{set}} --out-json {{out_json}}

# Sample ~200K English cards into out/plan.jsonl (shape floor + locale tiers).
[group('9-img-sampler')]
img-sample equinox_root="../equinox-cards" budget="200000" seed="7519" plan="out/plan.jsonl" summary="out/plan-summary.json":
    cargo run -p image-sampler --release -- sample --index-dir build/full_index --set ALL_SETS --equinox-root {{equinox_root}} --budget {{budget}} --full-locale-fraction 0.01 --fr-locale-fraction 0.10 --seed {{seed}} --out {{plan}} --out-summary {{summary}}

# Smaller sample for testing (5K cards).
[group('9-img-sampler')]
img-sample-test equinox_root="../equinox-cards" budget="2000" plan="out/plan-test.jsonl" summary="out/plan-test-summary.json":
    cargo run -p image-sampler --release -- sample --equinox-root {{equinox_root}} --budget {{budget}} --out {{plan}} --out-summary {{summary}}

# Resolve sampled cards' JSONs → out/plan-resolved.jsonl (locale → rel_path only).
[group('9-img-sampler')]
img-resolve equinox_root="../equinox-cards" plan="out/plan.jsonl" resolved="out/plan-resolved.jsonl" errors="out/resolve-errors.jsonl":
    cargo run -p image-sampler --release -- resolve-urls --plan {{plan}} --equinox-root {{equinox_root}} --out {{resolved}} --out-errors {{errors}}

# Download images from plan-resolved (resumable; skips existing files on disk).
[group('9-img-sampler')]
img-download-spot-check quality="75" plan="out/plan-resolved.jsonl" out_dir="out" concurrency="4" spot_check="5" seed="7519" rate="2":
    cargo run -p image-sampler --release -- download --plan {{plan}} --out-dir {{out_dir}}/compare-q{{quality}} --concurrency {{concurrency}} --spot-check-n {{spot_check}} --seed {{seed}} --images-per-second {{rate}} --quality {{quality}}

# Resume download after interrupt (no spot-check).
[group('9-img-sampler')]
img-download plan="out/plan-resolved.jsonl" out_dir="out" concurrency="5" seed="7519" rate="10":
    cargo run -p image-sampler --release -- download --plan {{plan}} --out-dir {{out_dir}} --concurrency {{concurrency}} --spot-check-n 0 --seed {{seed}} --images-per-second {{rate}}

# Full pipeline: sample → resolve → download (uses default 200K budget).
[group('9-img-sampler')]
img-pipeline equinox_root="../equinox-cards":
    just img-sample equinox_root={{equinox_root}}
    just img-resolve equinox_root={{equinox_root}}
    just img-download

