//! Append-only raw capture container.
//!
//! Each independently compressed block has a checksum. A missing trailer means
//! the process did not finish cleanly; valid earlier blocks remain readable.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use md_core::CaptureRecord;
use thiserror::Error;

const FILE_MAGIC: &[u8; 8] = b"MDRAW001";
const BLOCK_MAGIC: &[u8; 4] = b"MDBL";
const END_MAGIC: &[u8; 4] = b"MDEN";
const MAX_BLOCK_BYTES: usize = 64 * 1024 * 1024;
const MAX_COMPRESSED_BLOCK_BYTES: usize = MAX_BLOCK_BYTES + 1024 * 1024;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid capture header")]
    InvalidHeader,
    #[error("unsupported capture version {0}")]
    UnsupportedVersion(u16),
    #[error("invalid block marker")]
    InvalidBlock,
    #[error("block exceeds configured size limit")]
    OversizedBlock,
    #[error("block checksum mismatch")]
    ChecksumMismatch,
    #[error("invalid record: {0}")]
    InvalidRecord(#[from] serde_json::Error),
    #[error("compression error: {0}")]
    Compression(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureHeader {
    pub provider: String,
    pub symbol: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct WriterOptions {
    pub target_block_bytes: usize,
    pub compression_level: i32,
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self { target_block_bytes: 256 * 1024, compression_level: 3 }
    }
}

#[derive(Debug)]
pub struct RawWriter<W: Write> {
    out: W,
    pending: Vec<u8>,
    options: WriterOptions,
    record_count: u64,
    finished: bool,
}

impl<W: Write> RawWriter<W> {
    pub fn create(mut out: W, header: &CaptureHeader, options: WriterOptions) -> Result<Self, StorageError> {
        out.write_all(FILE_MAGIC)?;
        out.write_all(&1_u16.to_le_bytes())?;
        write_string(&mut out, &header.provider)?;
        write_string(&mut out, &header.symbol)?;
        write_string(&mut out, &header.session_id)?;
        Ok(Self { out, pending: Vec::new(), options, record_count: 0, finished: false })
    }

    pub fn append(&mut self, record: &CaptureRecord) -> Result<(), StorageError> {
        if self.finished { return Err(StorageError::InvalidBlock); }
        let encoded = serde_json::to_vec(record)?;
        let len = u32::try_from(encoded.len()).map_err(|_| StorageError::OversizedBlock)?;
        self.pending.extend_from_slice(&len.to_le_bytes());
        self.pending.extend_from_slice(&encoded);
        self.record_count += 1;
        if self.pending.len() >= self.options.target_block_bytes {
            self.flush_block()?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, StorageError> {
        self.flush_block()?;
        self.out.write_all(END_MAGIC)?;
        self.out.write_all(&self.record_count.to_le_bytes())?;
        self.out.flush()?;
        self.finished = true;
        Ok(self.out)
    }

    fn flush_block(&mut self) -> Result<(), StorageError> {
        if self.pending.is_empty() { return Ok(()); }
        let compressed = zstd::stream::encode_all(self.pending.as_slice(), self.options.compression_level)
            .map_err(|e| StorageError::Compression(e.to_string()))?;
        let raw_len = u32::try_from(self.pending.len()).map_err(|_| StorageError::OversizedBlock)?;
        let compressed_len = u32::try_from(compressed.len()).map_err(|_| StorageError::OversizedBlock)?;
        let crc = crc32fast::hash(&self.pending);
        self.out.write_all(BLOCK_MAGIC)?;
        self.out.write_all(&raw_len.to_le_bytes())?;
        self.out.write_all(&compressed_len.to_le_bytes())?;
        self.out.write_all(&crc.to_le_bytes())?;
        self.out.write_all(&compressed)?;
        self.pending.clear();
        Ok(())
    }
}

pub fn create_file(path: &Path, header: &CaptureHeader) -> Result<RawWriter<File>, StorageError> {
    let file = File::create(path)?;
    RawWriter::create(file, header, WriterOptions::default())
}

#[derive(Debug)]
pub struct ReadCapture {
    pub header: CaptureHeader,
    pub records: Vec<CaptureRecord>,
    pub complete: bool,
    pub truncated_bytes: u64,
    pub declared_record_count: Option<u64>,
}

pub fn read_file(path: &Path) -> Result<ReadCapture, StorageError> {
    let mut input = File::open(path)?;
    read_capture(&mut input)
}

pub fn read_capture<R: Read>(input: &mut R) -> Result<ReadCapture, StorageError> {
    let mut magic = [0_u8; 8];
    input.read_exact(&mut magic)?;
    if &magic != FILE_MAGIC { return Err(StorageError::InvalidHeader); }
    let mut version = [0_u8; 2];
    input.read_exact(&mut version)?;
    let version = u16::from_le_bytes(version);
    if version != 1 { return Err(StorageError::UnsupportedVersion(version)); }
    let header = CaptureHeader {
        provider: read_string(input)?,
        symbol: read_string(input)?,
        session_id: read_string(input)?,
    };
    let mut records = Vec::new();
    let mut complete = false;
    let mut truncated_bytes = 0_u64;
    let mut declared_record_count = None;
    loop {
        let mut marker = [0_u8; 4];
        match input.read_exact(&mut marker) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        if &marker == END_MAGIC {
            let mut count = [0_u8; 8];
            if input.read_exact(&mut count).is_err() { truncated_bytes = 8; break; }
            declared_record_count = Some(u64::from_le_bytes(count));
            complete = true;
            break;
        }
        if &marker != BLOCK_MAGIC { return Err(StorageError::InvalidBlock); }
        let mut fields = [0_u8; 12];
        if input.read_exact(&mut fields).is_err() { truncated_bytes = 12; break; }
        let raw_len = u32::from_le_bytes(fields[0..4].try_into().unwrap()) as usize;
        let compressed_len = u32::from_le_bytes(fields[4..8].try_into().unwrap()) as usize;
        let expected_crc = u32::from_le_bytes(fields[8..12].try_into().unwrap());
        if raw_len > MAX_BLOCK_BYTES || compressed_len > MAX_COMPRESSED_BLOCK_BYTES { return Err(StorageError::OversizedBlock); }
        let mut compressed = vec![0_u8; compressed_len];
        if input.read_exact(&mut compressed).is_err() {
            truncated_bytes = compressed_len as u64;
            break;
        }
        let raw = zstd::stream::decode_all(compressed.as_slice())
            .map_err(|e| StorageError::Compression(e.to_string()))?;
        if raw.len() != raw_len || crc32fast::hash(&raw) != expected_crc {
            return Err(StorageError::ChecksumMismatch);
        }
        let mut cursor = 0;
        while cursor < raw.len() {
            if cursor + 4 > raw.len() { return Err(StorageError::InvalidBlock); }
            let len = u32::from_le_bytes(raw[cursor..cursor+4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len > raw.len() { return Err(StorageError::InvalidBlock); }
            records.push(serde_json::from_slice(&raw[cursor..cursor+len])?);
            cursor += len;
        }
    }
    Ok(ReadCapture { header, records, complete, truncated_bytes, declared_record_count })
}

fn write_string<W: Write>(out: &mut W, value: &str) -> Result<(), StorageError> {
    let bytes = value.as_bytes();
    let len = u16::try_from(bytes.len()).map_err(|_| StorageError::OversizedBlock)?;
    out.write_all(&len.to_le_bytes())?;
    out.write_all(bytes)?;
    Ok(())
}

fn read_string<R: Read>(input: &mut R) -> Result<String, StorageError> {
    let mut len = [0_u8; 2];
    input.read_exact(&mut len)?;
    let mut bytes = vec![0_u8; u16::from_le_bytes(len) as usize];
    input.read_exact(&mut bytes)?;
    Ok(String::from_utf8(bytes).map_err(|_| StorageError::InvalidHeader)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_clean_trailer() {
        let header = CaptureHeader { provider: "binance".into(), symbol: "BTCUSDT".into(), session_id: "test".into() };
        let mut writer = RawWriter::create(Vec::new(), &header, WriterOptions { target_block_bytes: 1, compression_level: 1 }).unwrap();
        writer.append(&CaptureRecord::venue_payload("binance", "BTCUSDT", "btcusdt@trade", Some(10), 20, 1, br#"{"p":"1"}"#.to_vec())).unwrap();
        let bytes = writer.finish().unwrap();
        let result = read_capture(&mut bytes.as_slice()).unwrap();
        assert!(result.complete);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].payload, br#"{"p":"1"}"#);
        assert_eq!(result.declared_record_count, Some(1));
    }
}
