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

2. Run or build from the repository root

This repo is a Cargo workspace. Run commands from the root directory:

```
cargo run -p cli-indexer -- --help
cargo run -p uniques-http-api
```

Or build release binaries:

```
cargo build --release
.\target\release\cli-indexer.exe --help   # adjust to match your OS
.\target\release\uniques-http-api.exe
```

See [CLI Reference](./docs/cli-reference.md) for command-line examples.

## Deployment

A [Dockerfile](./Dockerfile) builds the HTTP server image with embedded index.

The expected process is to build the index first (per-set output in `build/sets_index/`, merged output in `build/full_index/ALL_SETS`) from the AlteredEquinox repositories, then the Docker image will embed a copy of it.