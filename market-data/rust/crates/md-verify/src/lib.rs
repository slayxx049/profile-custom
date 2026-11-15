//! Offline integrity checks for raw capture files.

use md_core::RecordKind;
use md_book::OrderBook;
use md_storage::{read_file, ReadCapture, StorageError};
use std::path::Path;

#[derive(Debug, Default)]
pub struct VerificationReport {
    pub records: usize,
    pub gaps: usize,
    pub sequence_holes: Vec<(u64, u64)>,
    pub complete: bool,
    pub count_matches: bool,
    pub truncated_bytes: u64,
    pub book_errors: Vec<String>,
}

pub fn verify(path: &Path) -> Result<VerificationReport, StorageError> {
    let capture = read_file(path)?;
    Ok(verify_capture(&capture))
}

pub fn verify_capture(capture: &ReadCapture) -> VerificationReport {
    let mut report = VerificationReport {
        records: capture.records.len(),
        complete: capture.complete,
        count_matches: capture.declared_record_count == Some(capture.records.len() as u64),
        truncated_bytes: capture.truncated_bytes,
        ..VerificationReport::default()
    };
    let mut previous = None;
    let mut book = OrderBook::default();
    for record in &capture.records {
        if record.kind == RecordKind::Gap {
            report.gaps += 1;
            book = OrderBook::default();
        }
        if record.kind == RecordKind::VenueSnapshot {
            if let Err(e) = book.apply_snapshot(&record.payload) {
                report.book_errors.push(format!("snapshot seq {}: {e}", record.ingest_seq));
                book = OrderBook::default();
            }
        } else if record.kind == RecordKind::VenuePayload && record.stream.contains("@depth") {
            let parsed: Result<serde_json::Value, _> = serde_json::from_slice(&record.payload);
            match parsed {
                Ok(value) => {
                    let data = value.get("data").unwrap_or(&value);
                    match serde_json::to_vec(data) {
                        Ok(raw) => if let Err(e) = book.apply_delta(&raw) {
                            report.book_errors.push(format!("depth seq {}: {e}", record.ingest_seq));
                            book = OrderBook::default();
                        },
                        Err(e) => report.book_errors.push(format!("depth seq {}: {e}", record.ingest_seq)),
                    }
                }
                Err(e) => report.book_errors.push(format!("depth seq {}: {e}", record.ingest_seq)),
            }
        }
        if let Some(prev) = previous {
            if record.ingest_seq != prev + 1 {
                report.sequence_holes.push((prev, record.ingest_seq));
            }
        }
        previous = Some(record.ingest_seq);
    }
    report
}

impl VerificationReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.complete && self.count_matches && self.sequence_holes.is_empty() && self.truncated_bytes == 0 && self.book_errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_core::CaptureRecord;
    use md_storage::CaptureHeader;

    #[test]
    fn detects_missing_sequence_numbers() {
        let capture = ReadCapture {
            header: CaptureHeader { provider: "x".into(), symbol: "Y".into(), session_id: "z".into() },
            records: vec![
                CaptureRecord::venue_payload("x","Y","s",None,1,1,vec![]),
                CaptureRecord::venue_payload("x","Y","s",None,3,3,vec![]),
            ],
            complete: true,
            truncated_bytes: 0,
            declared_record_count: Some(2),
        };
        let report = verify_capture(&capture);
        assert_eq!(report.sequence_holes, vec![(1,3)]);
        assert!(!report.is_clean());
    }
}
