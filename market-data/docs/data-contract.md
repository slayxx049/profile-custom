# Raw and normalized data contract

## Raw capture

Each capture file is self-describing and append-only.

- File magic: `MDRAW001`
- Container version: `u16 = 1`
- Header strings: provider, symbol, session identifier
- Block marker: `MDBL`
- Block fields: uncompressed byte length, compressed byte length, CRC32 of uncompressed bytes
- Block body: independently Zstandard-compressed sequence of length-prefixed JSON records
- Completion trailer: `MDEN` followed by the declared record count

The raw payload field retains the provider message bytes. Parsing belongs downstream. A capture without the completion trailer is incomplete, even if all earlier blocks are readable.

## Event envelope

Every record carries:

- `schema_version`
- `provider`
- `symbol`
- `stream`
- `kind` (`venue_payload`, `venue_snapshot`, or `gap`)
- `exchange_ts_ns` (nullable)
- `local_recv_ts_ns`
- `ingest_seq`
- `payload`
- `gap` (nullable)

The sequence is assigned locally, monotonically, per capture session. Local receive time is the local observation clock; exchange time is retained as provider-supplied provenance and is not used to repair ordering.

## Gaps and integrity

A known disconnect, reconnect, local overflow, or unavailable snapshot must not be silently represented as normal continuity. Gap records are explicit. A missing sequence is a verifier finding, not something to interpolate.

A clean trailer records the number of records written. The verifier compares this value with the decoded record count and checks sequence continuity.

## Normalized Parquet tier

The current normalizer emits separate Parquet files:

- `trades.parquet`
- `book_deltas.parquet`
- `book_snapshots.parquet`
- `gaps.parquet`

The first pass stores common provenance columns plus the original payload in a binary column. This preserves the source bytes and makes the output columnar, but it is not yet the reference project's fully typed event schema. Typed decimal price/quantity columns, nanosecond timestamp logical types, date partitioning, and provenance manifests remain acceptance items for Checkpoint 1.

## Provider boundary

Binance is the only configured adapter in this checkpoint. The SENSEX source must be selected independently and must not reuse Binance endpoints. The Cloudflare SENSEX Worker remains separate and unchanged.
