# rust-cards-api

Fast in-memory search over **Altered TCG Unique** cards: filter by abilities (`idGd`), mana cost, powers, faction, and more.

## How it works

This repo builds a compact binary index (~270 MB for all sets merged) and serves it from a small Rust HTTP API—no external database. See **[Architecture overview](docs/architecture.md)** for motivation, design, Roaring bitmaps, and `cards.bin`.

Notes that at this stage, this project only handles **Unique** characters. It does not serve and Common, Rare or Exalted cards. It could be expanded to serve them, but it was not its main purpose.

## Crates

| Directory | Description |
|-----------|-------------|
| [`alt-indexer/`](alt-indexer/) | Build and merge the index from card JSON |
| [`uniques-http-api/`](uniques-http-api/) | Load the index and expose the REST API |
| [`demo-ui/`](demo-ui/) | Browser demo UI (optional) |

## Documentation

- [Architecture overview](docs/architecture.md)
- [ALL_SETS index format](docs/ALL_SETS-index-format.md)
- [HTTP API spec](uniques-http-api/docs/api-spec.md)

## Getting started

1. Install Rust

Follow instructions at https://rust-lang.org/tools/install/

2. Run or build sub-project

First go to the sub-project you want to work with:
```
cd alt-indexer
```

Then you can either "run" (compile & run)
```
cargo run -- --help
```

Or "build" the project for release, before running it:
```
cargo build --release
.\target\release\alt-indexer.exe --help # adjust to match your OS
```

See [CLI Reference](./docs/cli-reference.md) for Command Line examples.

## Deployment

A [Dockerfile](./Dockerfile) builds the HTTP server image with embedded index.

The expected process is to build the index first from the AlteredEquinox repositories, then the Docker image will embed a copy of it.