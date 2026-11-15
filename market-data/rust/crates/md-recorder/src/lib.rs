//! Provider-neutral ingress sequencing and bounded handoff.

use std::sync::mpsc::{SyncSender, TrySendError};
use md_core::{CaptureRecord, GapCause, GapInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngressOutcome {
    Enqueued { ingest_seq: u64 },
    Dropped { ingest_seq: u64 },
}

#[derive(Debug)]
pub struct Ingress {
    provider: String,
    symbol: String,
    stream: String,
    next_seq: u64,
    last_good_ts_ns: i64,
    tx: SyncSender<CaptureRecord>,
}

impl Ingress {
    #[must_use]
    pub fn new(
        provider: impl Into<String>,
        symbol: impl Into<String>,
        stream: impl Into<String>,
        tx: SyncSender<CaptureRecord>,
    ) -> Self {
        Self { provider: provider.into(), symbol: symbol.into(), stream: stream.into(), next_seq: 1, last_good_ts_ns: 0, tx }
    }

    /// Stamp and enqueue one original venue message. A full queue is observable
    /// as a sequence hole; callers must stop treating the capture as continuous.
    pub fn accept(&mut self, payload: Vec<u8>, exchange_ts_ns: Option<i64>, local_recv_ts_ns: i64) -> Result<IngressOutcome, String> {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.checked_add(1).ok_or_else(|| "ingest sequence exhausted".to_owned())?;
        let record = CaptureRecord::venue_payload(
            self.provider.clone(), self.symbol.clone(), self.stream.clone(),
            exchange_ts_ns, local_recv_ts_ns, seq, payload
        );
        match self.tx.try_send(record) {
            Ok(()) => {
                self.last_good_ts_ns = local_recv_ts_ns;
                Ok(IngressOutcome::Enqueued { ingest_seq: seq })
            }
            Err(TrySendError::Full(_)) => Ok(IngressOutcome::Dropped { ingest_seq: seq }),
            Err(TrySendError::Disconnected(_)) => Err("capture writer disconnected".to_owned()),
        }
    }

    /// Create a gap record after reconnect or known transport discontinuity.
    pub fn gap(&mut self, cause: GapCause, local_recv_ts_ns: i64, detail: Option<String>) -> Result<u64, String> {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.checked_add(1).ok_or_else(|| "ingest sequence exhausted".to_owned())?;
        let record = CaptureRecord::gap(
            self.provider.clone(), self.symbol.clone(), self.stream.clone(),
            local_recv_ts_ns, seq,
            GapInfo { cause, last_good_ts_ns: self.last_good_ts_ns, detail }
        );
        self.tx.send(record).map_err(|_| "capture writer disconnected".to_owned())?;
        Ok(seq)
    }
}
