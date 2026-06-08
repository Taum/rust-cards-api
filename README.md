# rust-cards-api

Fast in-memory search over **Altered TCG Unique** cards: filter by abilities (`idGd`), mana cost, powers, faction, and more.

## How it works

This repo builds a compact binary index (~270 MB for all sets merged) and serves it from a small Rust HTTP API—no external database. See **[Architecture overview](docs/architecture.md)** for motivation, design, Roaring bitmaps, and `cards.bin`.

Notes that at this stage, this project only handles **Unique** characters. It does not serve and Common, Rare or Exalted cards. It could be expanded to serve them, but it was not its main purpose.

## Demo

Access the demo at https://taum.github.io/rust-cards-api/

## Crates


| Directory                                | Description                              |
| ---------------------------------------- | ---------------------------------------- |
| `[index-core/](index-core/)`             | Shared index library (build, query, types) |
| `[cli-indexer/](cli-indexer/)`           | CLI to build and merge the index from card JSON |
| `[uniques-http-api/](uniques-http-api/)` | Load the index and expose the REST API   |
| `[demo-ui/](demo-ui/)`                   | Browser demo UI (optional)               |


## Documentation

- [Architecture overview](docs/architecture.md)
- [ALL_SETS index format](docs/ALL_SETS-index-format.md)
- [HTTP API spec](docs/api-spec.md)

## Getting started

1. Install Rust

Follow instructions at [https://rust-lang.org/tools/install/](https://rust-lang.org/tools/install/)

2. Install [just](https://github.com/casey/just) (task runner; optional but recommended)

```bash
cargo install just
```

On Windows you can also use `winget install Casey.Just`.

3. Copy example local.env / local.toml files

Local environment and config files are ignored on Git, they allow developers to run configs to match their local dev environment.

```bash
cp uniques-http-api/config/local.toml.example uniques-http-api/config/local.toml
cp .env.local.template .env.local
```

4. Run from the repository root

Common tasks via `just`:

```bash
just              # list available commands
just api          # build and run the HTTP API (release)
just demo-ui      # run the Vite demo UI dev server
```

The API reads `uniques-http-api/config/local.toml` (copy from [`.local.toml.template`](uniques-http-api/config/local.toml.example)). You need a merged index at `build/` before the API can start — see **Pre-build index** section below and `just` recipes under `4-production`.

Equivalent Cargo commands:

```
cargo run -p uniques-http-api --release
cargo run -p cli-indexer -- --help
```

Or build release binaries:

```
cargo build --release
.\target\release\cli-indexer.exe --help   # adjust to match your OS
.\target\release\uniques-http-api.exe
```

See [CLI Reference](./docs/cli-reference.md) for `cli-indexer` command-line examples.

## Pre-built index

A pre-built index is available from https://storage.googleapis.com/taum-reunion-public/index/full_index.tar.zst

Save it into a `build` folder at the root of this repository (folder is Gitignored). The default local configuration of the API server will read from it.

## Deployment

A [Dockerfile](./Dockerfile) builds the HTTP server image.

The Dockerfile expects a production configuration in ./deployment/production.toml -- you can create an empty file if you do not need to do any overrides to the default configuration.