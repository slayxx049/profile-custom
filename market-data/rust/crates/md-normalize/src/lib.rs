//! Rebuildable columnar tier. Raw captures remain the source of truth.
//!
//! This first implementation writes one typed Parquet envelope per event
//! family. Provider payloads are retained as binary columns; venue-specific
//! decoding is deliberately a separate, versioned step.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, Int64Array, RecordBatch, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use md_core::{CaptureRecord, RecordKind};
use md_storage::{read_file, StorageError};
use parquet::arrow::ArrowWriter;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NormalizeError {
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Arrow: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error("Parquet: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
}

pub fn normalize_file(input: &Path, output_root: &Path) -> Result<Vec<PathBuf>, NormalizeError> {
    let capture = read_file(input)?;
    let mut groups: [Vec<CaptureRecord>; 4] = std::array::from_fn(|_| Vec::new());
    for record in capture.records {
        let index = match record.kind {
            RecordKind::VenueSnapshot => 2,
            RecordKind::Gap => 3,
            RecordKind::VenuePayload if record.stream.contains("@trade") => 0,
            RecordKind::VenuePayload if record.stream.contains("@depth") => 1,
            RecordKind::VenuePayload => 0,
        };
        groups[index].push(record);
    }

    let base = output_root
        .join(format!("provider={}", safe_component(&capture.header.provider)))
        .join(format!("symbol={}", safe_component(&capture.header.symbol)));
    fs::create_dir_all(&base)?;
    let names = ["trades", "book_deltas", "book_snapshots", "gaps"];
    let mut written = Vec::new();
    for (name, records) in names.into_iter().zip(groups.into_iter()) {
        if records.is_empty() { continue; }
        let path = base.join(format!("{name}.parquet"));
        write_dataset(&path, &records)?;
        written.push(path);
    }
    Ok(written)
}

fn write_dataset(path: &Path, records: &[CaptureRecord]) -> Result<(), NormalizeError> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ingest_seq", DataType::UInt64, false),
        Field::new("local_recv_ts_ns", DataType::Int64, false),
        Field::new("exchange_ts_ns", DataType::Int64, true),
        Field::new("provider", DataType::Utf8, false),
        Field::new("symbol", DataType::Utf8, false),
        Field::new("stream", DataType::Utf8, false),
        Field::new("record_kind", DataType::Utf8, false),
        Field::new("gap_cause", DataType::Utf8, true),
        Field::new("payload", DataType::Binary, false),
    ]));
    let seq: Vec<u64> = records.iter().map(|r| r.ingest_seq).collect();
    let local: Vec<i64> = records.iter().map(|r| r.local_recv_ts_ns).collect();
    let exchange: Vec<Option<i64>> = records.iter().map(|r| r.exchange_ts_ns).collect();
    let providers: Vec<&str> = records.iter().map(|r| r.provider.as_str()).collect();
    let symbols: Vec<&str> = records.iter().map(|r| r.symbol.as_str()).collect();
    let streams: Vec<&str> = records.iter().map(|r| r.stream.as_str()).collect();
    let kinds: Vec<&str> = records.iter().map(|r| match r.kind {
        RecordKind::VenuePayload => "venue_payload",
        RecordKind::VenueSnapshot => "venue_snapshot",
        RecordKind::Gap => "gap",
    }).collect();
    let causes: Vec<Option<&str>> = records.iter().map(|r| r.gap.as_ref().map(|g| match g.cause {
        md_core::GapCause::Disconnect => "disconnect",
        md_core::GapCause::SequenceGap => "sequence_gap",
        md_core::GapCause::LocalOverflow => "local_overflow",
        md_core::GapCause::RecorderRestart => "recorder_restart",
        md_core::GapCause::SnapshotUnavailable => "snapshot_unavailable",
    })).collect();
    let payloads: Vec<&[u8]> = records.iter().map(|r| r.payload.as_slice()).collect();

    let columns: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(seq)),
        Arc::new(Int64Array::from(local)),
        Arc::new(Int64Array::from(exchange)),
        Arc::new(StringArray::from(providers)),
        Arc::new(StringArray::from(symbols)),
        Arc::new(StringArray::from(streams)),
        Arc::new(StringArray::from(kinds)),
        Arc::new(StringArray::from(causes)),
        Arc::new(BinaryArray::from(payloads)),
    ];
    let batch = RecordBatch::try_new(schema.clone(), columns)?;
    let file = File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, None)?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

fn safe_component(value: &str) -> String {
    value.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
