# Market Data Pipeline

A read-only market-data engineering project inspired by the separation of concerns in [alienxviking/quant](https://github.com/alienxviking/quant).

This is an independent implementation. It does not copy source files from that repository. The reference repository does not currently expose a licence file authorizing code reuse.

## Scope

The first implementation is limited to data engineering:

- ingest public market data;
- timestamp and sequence incoming records;
- preserve raw captures before transformation;
- normalize captures into columnar Parquet datasets;
- verify continuity and data integrity;
- expose read-only status and diagnostics.

No strategy engine, order routing, trade execution, portfolio simulation, or trading signals are included.

## Provider separation

| Adapter | Instrument family | Data source | Status |
|---|---|---|---|
| `md-binance` | Crypto (for example BTCUSDT, ETHUSDT) | Binance public market-data WebSocket; REST snapshot where the stream requires an anchor | Planned |
| `md-sensex` | Indian equity index | Separate SENSEX provider adapter; provider selection and API access must be verified | Not configured |

The two adapters must never share an upstream provider configuration. Binance is not a SENSEX data source. The existing Cloudflare SENSEX Worker and dashboard remain untouched by this pipeline.

## Pipeline

```text
Provider adapter
      |
      v
Ingress timestamp + sequence
      |
      v
Append-only raw capture
      |
      +------> Capture diagnostics
      |
      v
Offline validation
      |
      v
Normalization
      |
      v
Parquet datasets
      |
      v
Read-only inspection
```

Raw capture is the source of truth. Normalized data is reproducible output and must be safe to rebuild.

## Rust workspace

```text
market-data/
  rust/
    Cargo.toml
    crates/
      md-core/       provider-neutral capture event contract
      md-storage/    compressed, checksummed raw blocks
      md-recorder/   provider-neutral ingress and sequencing
      md-binance/    Binance public WebSocket + REST snapshot adapter
      md-book/       fixed-point order-book reconstruction
      md-normalize/  event-family Parquet output
      md-verify/     offline capture and depth-chain verifier
  docs/
    architecture.md
    data-contract.md
    provider-matrix.md
```

Crate names and interfaces are project-local; they are not copied source code.

## Checkpoint 1 — reference pipeline reproduction

This is the baseline branch. It is not considered passed until deterministic offline fixtures exercise storage, verification, reconstruction, normalization, and Parquet read-back. CI must not depend on a live exchange feed.

Implemented in this branch:
- [x] Provider-neutral event contract with exchange and local receive timestamps
- [x] Per-session ingest sequence
- [x] Bounded ingress handoff
- [x] Independently compressed raw blocks with CRC checks
- [x] Explicit clean-close trailer and interrupted-capture detection
- [x] Binance public trade and 100 ms depth stream adapter
- [x] Binance REST depth snapshot capture on connect and hourly
- [x] Fixed-point order-book reconstruction and update-id continuity checks
- [x] Offline capture verifier
- [x] Parquet output split into trades, book deltas, snapshots, and gaps

Still required before Checkpoint 1 passes:
- [ ] Run `cargo fmt --all`, `cargo test --workspace`, and `cargo clippy --workspace`
- [ ] Run a short public-data capture and verify its trailer and sequence
- [ ] Normalize the capture and read back all Parquet datasets
- [ ] Add deterministic end-to-end fixtures for reconnects, dropped messages, and torn tails
- [ ] Add queue/throughput/latency metrics and robust pending-gap handling
- [ ] Add calendar/session partitioning and capture catalog metadata
- [ ] Confirm Parquet schema and normalized payload fields against the reference contract
- [ ] Choose and verify a separate SENSEX provider; no adapter is configured yet

The Parquet files currently retain the original payload as a binary column alongside event metadata. Full venue-specific typed column extraction is still pending; this is not yet equivalent to the reference normalized schema.

## Build and test

Run from this directory:

```bash
cd market-data/rust
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets
```

Live provider capture is deliberately excluded from the automated validation workflow. Use deterministic fixtures for CI; do not treat a network capture as proof of pipeline correctness.\n\nThe adapter's local invocation interface (not part of CI) is:

```bash
cargo run --release -p md-binance --bin record -- BTCUSDT ../../data 60
```

Verify and normalize the generated capture:

```bash
cargo run --release -p md-verify --bin verify -- ../../data/BTCUSDT-<session>.mdraw.zst
cargo run --release -p md-normalize --bin normalize -- ../../data/BTCUSDT-<session>.mdraw.zst ../../normalized
```

This project is limited to offline software-architecture validation in this checkpoint. The automated tests must not connect to external market-data services.

## Reference

Architecture study: [alienxviking/quant](https://github.com/alienxviking/quant).

The reference project uses Binance WebSocket ingestion, durable raw capture, a separate normalization stage, Parquet storage, and offline verification. Its trading and execution layers are intentionally outside this project's scope.
