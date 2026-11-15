# Market Data Pipeline

A read-only market-data engineering project inspired by the separation of concerns in [alienxviking/quant](https://github.com/alienxviking/quant).

This is an independent implementation plan. It does not copy source files from that repository. The reference repository does not currently expose a licence file authorizing code reuse.

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

## Planned workspace

```text
market-data/
  crates/
    md-core/       shared event types and time contract
    md-storage/    framed raw capture and recovery
    md-recorder/   provider-neutral ingress and writer
    md-binance/    Binance-specific public market-data adapter
    md-sensex/     independent SENSEX provider adapter
    md-normalize/  raw capture to ordered Parquet datasets
    md-verify/     offline integrity checks
    md-meta/       optional capture metadata
  docs/
    architecture.md
    data-contract.md
    provider-matrix.md
```

Crate names and interfaces are project-local; they are not copied source code.

## Implementation status

This branch establishes the architecture and provider boundaries first. It is not yet a working recorder. A component is only marked complete after its implementation is tested against its acceptance criteria.

- [x] Reference architecture reviewed
- [x] Separate provider boundaries defined
- [x] Trading and execution components excluded
- [ ] Core event contract
- [ ] Raw capture writer and recovery
- [ ] Binance public-data adapter
- [ ] SENSEX provider selection and adapter
- [ ] Normalizer and Parquet read-back
- [ ] Offline verifier
- [ ] End-to-end test and deployment

## Reference

Architecture study: [alienxviking/quant](https://github.com/alienxviking/quant).

The reference project uses Binance WebSocket ingestion, durable raw capture, a separate normalization stage, Parquet storage, and offline verification. Its trading and execution layers are intentionally outside this project's scope.
