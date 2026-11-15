# Provider Matrix

| Data family | Adapter | Intended transport | Current status | Important constraint |
|---|---|---|---|---|
| Crypto spot | `md-binance` | Public WebSocket; REST snapshot only when required by the selected stream | Planned | Binance is only the crypto source |
| BSE SENSEX | `md-sensex` | To be selected after access and terms are verified | Unconfigured | Must not be routed through Binance |

## SENSEX acceptance checklist

A provider is not accepted merely because a web page displays an index value. Before implementation, verify:

- an official or clearly documented API;
- whether API access is included in a free tier;
- whether data is real-time or delayed, and by how much;
- whether WebSocket streaming is actually available;
- rate limits, redistribution terms, and attribution requirements;
- timestamp semantics and market-holiday behavior.

The existing Cloudflare Worker currently queries Yahoo Finance's undocumented chart endpoint. It is an unofficial fallback and must remain clearly labelled as such. It is not the provider for the new pipeline.
