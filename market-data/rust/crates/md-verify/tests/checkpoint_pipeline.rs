use md_core::CaptureRecord;
use md_normalize::normalize_file;
use md_storage::{CaptureHeader, RawWriter, WriterOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::{self, File};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn synthetic_capture_verifies_normalizes_and_reads_back_all_datasets() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "md-checkpoint-fixture-{}-{unique}",
        std::process::id()
    ));
    let raw_dir = root.join("raw");
    let output_dir = root.join("normalized");
    fs::create_dir_all(&raw_dir).unwrap();
    let raw_path = raw_dir.join("fixture.mdraw");

    let header = CaptureHeader {
        provider: "fixture".into(),
        symbol: "SAMPLE".into(),
        session_id: "checkpoint-1".into(),
    };
    let file = File::create(&raw_path).unwrap();
    let mut writer = RawWriter::create(file, &header, WriterOptions::default()).unwrap();
    writer
        .append(&CaptureRecord::venue_snapshot(
            "fixture",
            "SAMPLE",
            "sample@snapshot",
            1,
            1,
            br#"{"lastUpdateId":100,"bids":[["10.00","2.0"]],"asks":[["11.00","3.0"]]}"#.to_vec(),
        ))
        .unwrap();
    writer
        .append(&CaptureRecord::venue_payload(
            "fixture",
            "SAMPLE",
            "sample@trade",
            Some(2),
            2,
            2,
            br#"{"event":"trade","price":"10.50","quantity":"1.0"}"#.to_vec(),
        ))
        .unwrap();
    writer
        .append(&CaptureRecord::venue_payload(
            "fixture",
            "SAMPLE",
            "sample@depth",
            Some(3),
            3,
            3,
            br#"{"stream":"sample@depth","data":{"U":101,"u":101,"b":[["10.00","2.5"]],"a":[]}}"#
                .to_vec(),
        ))
        .unwrap();
    writer.finish().unwrap();

    let report = md_verify::verify(&raw_path).unwrap();
    assert!(
        report.is_clean(),
        "fixture verification failed: gaps={}, holes={:?}, book_errors={:?}",
        report.gaps,
        report.sequence_holes,
        report.book_errors
    );

    let paths = normalize_file(&raw_path, &output_dir).unwrap();
    assert_eq!(
        paths.len(),
        3,
        "expected trades, book_deltas, and book_snapshots"
    );
    for expected in [
        "trades.parquet",
        "book_deltas.parquet",
        "book_snapshots.parquet",
    ] {
        let path = paths
            .iter()
            .find(|p| p.file_name().unwrap() == expected)
            .unwrap();
        let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap()).unwrap();
        assert_eq!(
            builder.metadata().file_metadata().num_rows(),
            1,
            "{expected} row count"
        );
        let names: Vec<String> = builder
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect();
        for required in [
            "ingest_seq",
            "provider",
            "symbol",
            "stream",
            "record_kind",
            "payload",
        ] {
            assert!(
                names.iter().any(|name| name == required),
                "{expected} missing {required}"
            );
        }
        let batches = builder
            .build()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            batches.iter().map(|batch| batch.num_rows()).sum::<usize>(),
            1
        );
    }

    fs::remove_dir_all(root).unwrap();
}
