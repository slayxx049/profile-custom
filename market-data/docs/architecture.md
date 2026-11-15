# Architecture

## Design objective

Build a provider-neutral, read-only market-data pipeline. Provider-specific protocol handling must stop at the adapter boundary. Storage, sequencing, validation, and normalization must not know which exchange produced a record.

## Data path

1. **Provider adapter** receives a provider message and preserves the original payload.
2. **Ingress** assigns a monotonically increasing local sequence and records both provider time (when supplied) and local receive time.
3. **Raw storage** writes records before any lossy transformation.
4. **Verifier** checks sequence continuity, capture completeness, parseability, and provider-specific continuity rules where applicable.
5. **Normalizer** converts valid raw records into typed, ordered datasets.
6. **Parquet tier** stores normalized records for efficient offline analysis.
7. **Inspector** reads capture and normalized output without modifying either.

## Provider isolation

### Binance adapter

The crypto adapter owns Binance-specific WebSocket message decoding, connection lifecycle, and any REST snapshot operation needed to anchor an incremental stream. No Binance payload types should leak into the shared event model.

### SENSEX adapter

The SENSEX adapter is a separate integration. It must use a provider that explicitly supports the required access method and terms. The existing Yahoo-based Worker is an unofficial quote fallback, not an exchange-authorized feed and not a substitute for a verified streaming API.

Until a SENSEX provider and its API access are verified, this adapter remains unconfigured. No Binance endpoint or credential is reused for SENSEX.

## Event contract

Every normalized event should retain:

- provider identifier;
- instrument identifier;
- provider timestamp, if supplied;
- local receive timestamp;
- local ingest sequence;
- event type;
- original raw-record reference.

Dispatch and ordering must use local receive time plus the sequence tie-breaker. Provider timestamps are retained as data, not trusted as the local ordering clock.

## Failure semantics

A missing sequence, disconnect, failed snapshot, malformed message, or provider reset must be visible as a diagnostic/gap record. The pipeline must not silently bridge missing data or label an incomplete capture as complete.

A cleanly closed capture receives an explicit completion marker. A process interruption must remain distinguishable from a clean close.

## Non-goals

This project does not implement strategies, trading signals, risk limits, simulated fills, order placement, or broker connectivity.
