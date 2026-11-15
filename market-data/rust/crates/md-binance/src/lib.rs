//! Binance public market-data adapter.
//!
//! This crate owns Binance protocol details only. The shared recorder and
//! storage crates do not parse Binance payloads or depend on a network runtime.

use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use md_core::{CaptureRecord, GapCause};
use md_recorder::Ingress;
use md_storage::{CaptureHeader, RawWriter, WriterOptions};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;

const WS_BASE: &str = "wss://stream.binance.com:9443";
const REST_BASE: &str = "https://api.binance.com";
const QUEUE_CAPACITY: usize = 8192;

#[derive(Debug)]
pub enum AdapterError {
    Io(std::io::Error),
    Http(reqwest::Error),
    WebSocket(tokio_tungstenite::tungstenite::Error),
    Storage(md_storage::StorageError),
    Writer(String),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Http(e) => write!(f, "REST: {e}"),
            Self::WebSocket(e) => write!(f, "WebSocket: {e}"),
            Self::Storage(e) => write!(f, "storage: {e}"),
            Self::Writer(e) => write!(f, "writer: {e}"),
        }
    }
}

impl std::error::Error for AdapterError {}
impl From<std::io::Error> for AdapterError { fn from(e: std::io::Error) -> Self { Self::Io(e) } }
impl From<reqwest::Error> for AdapterError { fn from(e: reqwest::Error) -> Self { Self::Http(e) } }
impl From<tokio_tungstenite::tungstenite::Error> for AdapterError { fn from(e: tokio_tungstenite::tungstenite::Error) -> Self { Self::WebSocket(e) } }
impl From<md_storage::StorageError> for AdapterError { fn from(e: md_storage::StorageError) -> Self { Self::Storage(e) } }

fn now_ns() -> i64 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    i64::try_from(nanos).unwrap_or(i64::MAX)
}

fn session_id() -> String {
    format!("{}-{}", std::process::id(), now_ns())
}

fn stream_url(symbol: &str) -> String {
    let s = symbol.to_ascii_lowercase();
    format!("{WS_BASE}/stream?streams={s}@trade/{s}@depth@100ms")
}

async fn fetch_depth_snapshot(symbol: &str) -> Result<Vec<u8>, AdapterError> {
    let url = format!("{REST_BASE}/api/v3/depth?symbol={symbol}&limit=5000");
    let response = reqwest::Client::new().get(url).timeout(Duration::from_secs(15)).send().await?.error_for_status()?;
    Ok(response.bytes().await?.to_vec())
}

fn exchange_timestamp(payload: &[u8]) -> Option<i64> {
    let value: Value = serde_json::from_slice(payload).ok()?;
    let data = value.get("data").unwrap_or(&value);
    let millis = data.get("E").or_else(|| data.get("T"))?.as_i64()?;
    millis.checked_mul(1_000_000)
}

fn stream_name(payload: &[u8], symbol: &str) -> String {
    let value: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
    value.get("stream").and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}@unknown", symbol.to_ascii_lowercase()))
}

/// Capture public trades and 100 ms depth updates until Ctrl-C or the optional
/// duration limit. Raw WebSocket payloads are stored unchanged.
pub async fn run_capture(symbol: String, root: PathBuf, seconds: Option<u64>) -> Result<PathBuf, AdapterError> {
    let symbol = symbol.to_ascii_uppercase();
    fs::create_dir_all(&root)?;
    let id = session_id();
    let path = root.join(format!("{symbol}-{id}.mdraw.zst"));
    let file = File::create(&path)?;
    let header = CaptureHeader { provider: "binance".into(), symbol: symbol.clone(), session_id: id };
    let writer = RawWriter::create(file, &header, WriterOptions::default())?;
    let (tx, rx) = sync_channel::<CaptureRecord>(QUEUE_CAPACITY);
    let writer_thread = thread::Builder::new().name("md-raw-writer".into()).spawn(move || -> Result<(), String> {
        let mut writer = writer;
        while let Ok(record) = rx.recv() {
            writer.append(&record).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())?;
        Ok(())
    })?;

    let mut ingress = Ingress::new("binance", symbol.clone(), "market", tx);
    let deadline = seconds.map(|s| tokio::time::Instant::now() + Duration::from_secs(s));
    let mut first_connection = true;

    'capture: loop {
        if deadline.is_some_and(|d| tokio::time::Instant::now() >= d) { break; }
        let (mut socket, _) = match tokio_tungstenite::connect_async(stream_url(&symbol)).await {
            Ok(pair) => pair,
            Err(e) => {
                if !first_connection {
                    let _ = ingress.gap(GapCause::Disconnect, now_ns(), Some(e.to_string()));
                }
                first_connection = false;
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };
        first_connection = false;

        match fetch_depth_snapshot(&symbol).await {
            Ok(snapshot) => { ingress.accept_snapshot_on_stream("rest:depth-snapshot", snapshot, now_ns()).map_err(AdapterError::Writer)?; }
            Err(e) => {
                let _ = ingress.gap(GapCause::SnapshotUnavailable, now_ns(), Some(e.to_string()));
            }
        }

        let mut snapshot_interval = tokio::time::interval(Duration::from_secs(60 * 60));
        snapshot_interval.tick().await;
        let reconnect_at = tokio::time::Instant::now() + Duration::from_secs(23 * 60 * 60 + 50 * 60);
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break 'capture,
                _ = async {
                    if let Some(d) = deadline { tokio::time::sleep_until(d).await; }
                    else { std::future::pending::<()>().await; }
                } => break 'capture,
                _ = tokio::time::sleep_until(reconnect_at) => {
                    let _ = ingress.gap(GapCause::Disconnect, now_ns(), Some("scheduled reconnect before venue stream lifetime cap".into()));
                    break;
                },
                _ = snapshot_interval.tick() => {
                    match fetch_depth_snapshot(&symbol).await {
                        Ok(snapshot) => { ingress.accept_snapshot_on_stream("rest:depth-snapshot", snapshot, now_ns()).map_err(AdapterError::Writer)?; }
                        Err(e) => { let _ = ingress.gap(GapCause::SnapshotUnavailable, now_ns(), Some(e.to_string())); }
                    }
                },
                next = socket.next() => {
                    match next {
                        Some(Ok(Message::Text(text))) => {
                            let payload = text.as_bytes().to_vec();
                            let stream = stream_name(&payload, &symbol);
                            let ts = exchange_timestamp(&payload);
                            let local = now_ns();
                            // Keep the combined-stream envelope exactly as received.
                            let outcome = ingress.accept_stream(stream, payload, ts, local).map_err(AdapterError::Writer)?;
                            if matches!(outcome, md_recorder::IngressOutcome::Dropped { .. }) {
                                // The missing sequence is intentionally visible to the verifier.
                            }
                        }
                        Some(Ok(Message::Binary(bytes))) => {
                            let payload = bytes.to_vec();
                            let ts = exchange_timestamp(&payload);
                            let stream = stream_name(&payload, &symbol);
                            ingress.accept_stream(stream, payload, ts, now_ns()).map_err(AdapterError::Writer)?;
                        }
                        Some(Ok(Message::Ping(bytes))) => { socket.send(Message::Pong(bytes)).await?; }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Ok(_)) => {}
                        Some(Err(_e)) => { break; }
                    }
                }
            }
        }
        if deadline.is_some_and(|d| tokio::time::Instant::now() >= d) { break; }
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = ingress.gap(GapCause::Disconnect, now_ns(), Some("reconnecting after stream close".into()));
    }

    drop(ingress);
    // Dropping the ingress closes the sender and lets the writer seal the trailer.
    writer_thread.join().map_err(|_| AdapterError::Writer("writer thread panicked".into()))?
        .map_err(AdapterError::Writer)?;
    Ok(path)
}
