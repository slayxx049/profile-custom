//! Deterministic reconstruction of an incremental order book.
//!
//! Prices and quantities are parsed into fixed-point integers. No floating
//! point arithmetic is used. The module consumes raw Binance snapshot/delta
//! JSON but emits provider-neutral book state.

use std::collections::BTreeMap;
use thiserror::Error;

const SCALE: i128 = 100_000_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BookError {
    #[error("invalid JSON payload: {0}")]
    Json(String),
    #[error("missing or invalid field: {0}")]
    Field(&'static str),
    #[error("invalid decimal value: {0}")]
    Decimal(String),
    #[error("delta sequence discontinuity: expected update {expected}, got range {first}..={last}")]
    SequenceGap { expected: u64, first: u64, last: u64 },
    #[error("delta arrived before a snapshot")]
    Unanchored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookLevel {
    pub price_scaled: i64,
    pub quantity_scaled: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderBook {
    pub last_update_id: Option<u64>,
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
}

impl OrderBook {
    pub fn apply_snapshot(&mut self, raw: &[u8]) -> Result<(), BookError> {
        let value: serde_json::Value = serde_json::from_slice(raw).map_err(|e| BookError::Json(e.to_string()))?;
        let update_id = value.get("lastUpdateId").and_then(|v| v.as_u64()).ok_or(BookError::Field("lastUpdateId"))?;
        let bids = parse_levels(value.get("bids").ok_or(BookError::Field("bids"))?)?;
        let asks = parse_levels(value.get("asks").ok_or(BookError::Field("asks"))?)?;
        self.bids.clear();
        self.asks.clear();
        for (price, qty) in bids { if qty > 0 { self.bids.insert(price, qty); } }
        for (price, qty) in asks { if qty > 0 { self.asks.insert(price, qty); } }
        self.last_update_id = Some(update_id);
        Ok(())
    }

    pub fn apply_delta(&mut self, raw: &[u8]) -> Result<(), BookError> {
        let snapshot_id = self.last_update_id.ok_or(BookError::Unanchored)?;
        let value: serde_json::Value = serde_json::from_slice(raw).map_err(|e| BookError::Json(e.to_string()))?;
        let first = value.get("U").and_then(|v| v.as_u64()).ok_or(BookError::Field("U"))?;
        let last = value.get("u").and_then(|v| v.as_u64()).ok_or(BookError::Field("u"))?;
        if last <= snapshot_id { return Ok(()); }
        let expected = snapshot_id + 1;
        if first > expected || last < expected {
            return Err(BookError::SequenceGap { expected, first, last });
        }
        let bids = parse_levels(value.get("b").ok_or(BookError::Field("b"))?)?;
        let asks = parse_levels(value.get("a").ok_or(BookError::Field("a"))?)?;
        apply_levels(&mut self.bids, bids);
        apply_levels(&mut self.asks, asks);
        self.last_update_id = Some(last);
        Ok(())
    }

    #[must_use]
    pub fn best_bid(&self) -> Option<BookLevel> {
        self.bids.iter().next_back().map(|(p,q)| BookLevel { price_scaled:*p, quantity_scaled:*q })
    }

    #[must_use]
    pub fn best_ask(&self) -> Option<BookLevel> {
        self.asks.iter().next().map(|(p,q)| BookLevel { price_scaled:*p, quantity_scaled:*q })
    }

    #[must_use]
    pub fn bid_levels(&self) -> usize { self.bids.len() }

    #[must_use]
    pub fn ask_levels(&self) -> usize { self.asks.len() }
}

fn parse_levels(value: &serde_json::Value) -> Result<Vec<(i64, i64)>, BookError> {
    let rows = value.as_array().ok_or(BookError::Field("levels"))?;
    rows.iter().map(|row| {
        let pair = row.as_array().ok_or(BookError::Field("level pair"))?;
        let price = pair.first().and_then(|v| v.as_str()).ok_or(BookError::Field("price"))?;
        let qty = pair.get(1).and_then(|v| v.as_str()).ok_or(BookError::Field("quantity"))?;
        Ok((parse_fixed(price)?, parse_fixed(qty)?))
    }).collect()
}

fn parse_fixed(text: &str) -> Result<i64, BookError> {
    let negative = text.starts_with('-');
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let mut parts = unsigned.split('.');
    let whole = parts.next().unwrap_or("0").parse::<i128>().map_err(|_| BookError::Decimal(text.into()))?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some() || fraction.len() > 8 || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return Err(BookError::Decimal(text.into()));
    }
    let frac = if fraction.is_empty() { 0 } else {
        fraction.parse::<i128>().map_err(|_| BookError::Decimal(text.into()))? * 10_i128.pow((8 - fraction.len()) as u32)
    };
    let raw = whole.checked_mul(SCALE).and_then(|v| v.checked_add(frac)).ok_or_else(|| BookError::Decimal(text.into()))?;
    let signed = if negative { -raw } else { raw };
    i64::try_from(signed).map_err(|_| BookError::Decimal(text.into()))
}

fn apply_levels(book: &mut BTreeMap<i64, i64>, levels: Vec<(i64, i64)>) {
    for (price, qty) in levels {
        if qty == 0 { book.remove(&price); } else { book.insert(price, qty); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_and_contiguous_delta_reconstruct_top_of_book() {
        let mut book = OrderBook::default();
        book.apply_snapshot(br#"{"lastUpdateId":100,"bids":[["100.00","2.0"]],"asks":[["101.00","3.0"]]}"#).unwrap();
        book.apply_delta(br#"{"U":101,"u":101,"b":[["100.00","1.5"]],"a":[["101.00","0"]]}"#).unwrap();
        assert_eq!(book.last_update_id, Some(101));
        assert_eq!(book.best_bid().unwrap().quantity_scaled, 150_000_000);
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn refuses_unanchored_or_discontinuous_deltas() {
        let mut book = OrderBook::default();
        assert_eq!(book.apply_delta(br#"{"U":1,"u":1,"b":[],"a":[]}"#), Err(BookError::Unanchored));
        book.apply_snapshot(br#"{"lastUpdateId":10,"bids":[],"asks":[]}"#).unwrap();
        assert!(matches!(book.apply_delta(br#"{"U":12,"u":12,"b":[],"a":[]}"#), Err(BookError::SequenceGap { .. })));
    }
}
