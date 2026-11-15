//! Provider-neutral market-data contract.
//! Venue payloads remain opaque here; decoding belongs in provider adapters.

use serde::{Deserialize, Serialize};

pub const EVENT_SCHEMA_VERSION: u16 = 1;

/// Event category used by the raw recorder and downstream normalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    VenuePayload,
    VenueSnapshot,
    Gap,
}

/// Why a stream has a known discontinuity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapCause {
    Disconnect,
    SequenceGap,
    LocalOverflow,
    RecorderRestart,
    SnapshotUnavailable,
}

/// A single immutable observation captured at ingress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureRecord {
    pub schema_version: u16,
    pub provider: String,
    pub symbol: String,
    pub stream: String,
    pub kind: RecordKind,
    /// Provider timestamp in Unix nanoseconds, if supplied by the source.
    pub exchange_ts_ns: Option<i64>,
    /// Local receive timestamp in Unix nanoseconds.
    pub local_recv_ts_ns: i64,
    /// Monotonic sequence assigned by this recorder session.
    pub ingest_seq: u64,
    /// Original provider bytes, base64 encoded by serde_json.
    pub payload: Vec<u8>,
    pub gap: Option<GapInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapInfo {
    pub cause: GapCause,
    pub last_good_ts_ns: i64,
    pub detail: Option<String>,
}

impl CaptureRecord {
    #[must_use]
    pub fn venue_payload(
        provider: impl Into<String>,
        symbol: impl Into<String>,
        stream: impl Into<String>,
        exchange_ts_ns: Option<i64>,
        local_recv_ts_ns: i64,
        ingest_seq: u64,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION,
            provider: provider.into(),
            symbol: symbol.into(),
            stream: stream.into(),
            kind: RecordKind::VenuePayload,
            exchange_ts_ns,
            local_recv_ts_ns,
            ingest_seq,
            payload,
            gap: None,
        }
    }

    #[must_use]
    #[must_use]
    pub fn venue_snapshot(
        provider: impl Into<String>,
        symbol: impl Into<String>,
        stream: impl Into<String>,
        local_recv_ts_ns: i64,
        ingest_seq: u64,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION,
            provider: provider.into(),
            symbol: symbol.into(),
            stream: stream.into(),
            kind: RecordKind::VenueSnapshot,
            exchange_ts_ns: None,
            local_recv_ts_ns,
            ingest_seq,
            payload,
            gap: None,
        }
    }

    pub fn gap(
        provider: impl Into<String>,
        symbol: impl Into<String>,
        stream: impl Into<String>,
        local_recv_ts_ns: i64,
        ingest_seq: u64,
        info: GapInfo,
    ) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION,
            provider: provider.into(),
            symbol: symbol.into(),
            stream: stream.into(),
            kind: RecordKind::Gap,
            exchange_ts_ns: None,
            local_recv_ts_ns,
            ingest_seq,
            payload: Vec::new(),
            gap: Some(info),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_payload_round_trips_without_interpretation() {
        let raw = br#"{"e":"trade","p":"68123.45000000"}"#.to_vec();
        let event = CaptureRecord::venue_payload(
            "binance", "BTCUSDT", "btcusdt@trade", Some(10), 20, 1, raw.clone()
        );
        let json = serde_json::to_vec(&event).unwrap();
        let decoded: CaptureRecord = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.payload, raw);
        assert_eq!(decoded.ingest_seq, 1);
    }
}
