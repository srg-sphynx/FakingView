//! Kite's binary quote stream. These are ticks, not candles: chart history continues
//! to use the historical API, avoiding partial OHLC/volume after a late subscription.

use crate::model::watchlist::quote::{SourceQuote, VolumeKind};
use crate::service::keychain::api_keys::kite_credentials;
use crate::service::market::kite::{encode, instrument_tokens};
use crate::service::net::ws::{Backoff, Frame, Keepalive, Session};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const HOST: &str = "ws.kite.trade";
const MAX_INSTRUMENTS: usize = 3_000;
const CREDENTIAL_CHECK: Duration = Duration::from_secs(1);

/// Runs on the shared quote socket's worker. Reads credentials on the worker, then
/// uses the common proxy-aware connection, health checks and reconnect pacing.
pub(crate) fn run(
    products: &[String],
    shutdown: &AtomicBool,
    mut on_quote: impl FnMut(String, SourceQuote) -> bool,
) {
    let mut backoff = Backoff::new();
    while !shutdown.load(Ordering::Acquire) {
        let Some(credentials) = kite_credentials() else {
            backoff.wait(shutdown);
            continue;
        };
        let Ok(instruments) = instrument_tokens(products) else {
            backoff.wait(shutdown);
            continue;
        };
        let instruments: HashMap<_, _> = instruments.into_iter().take(MAX_INSTRUMENTS).collect();
        if instruments.is_empty() {
            backoff.wait(shutdown);
            continue;
        }
        let tokens: Vec<u32> = instruments.keys().copied().collect();
        let session = session(&credentials, &tokens);
        let catalog_day = india_day();
        let mut checked = Instant::now();
        let mut credentials_unchanged = true;
        let mut listening = true;
        session.run_frames_while(
            shutdown,
            || {
                if checked.elapsed() >= CREDENTIAL_CHECK {
                    credentials_unchanged = kite_credentials().as_ref() == Some(&credentials)
                        && india_day() == catalog_day;
                    checked = Instant::now();
                }
                credentials_unchanged
            },
            |frame| {
                if let Frame::Binary(bytes) = frame
                    && let Some(ticks) = decode(bytes)
                {
                    for (token, quote) in ticks {
                        if let Some(product) = instruments.get(&token)
                            && !on_quote(product.clone(), quote)
                        {
                            listening = false;
                            return false;
                        }
                    }
                }
                true
            },
        );
        if !listening {
            return;
        }
        // A credential change needs a new URL; ordinary network errors are retried
        // by Session with its bounded exponential backoff.
        backoff.reset();
    }
}

fn session(credentials: &(String, String), tokens: &[u32]) -> Session {
    Session {
        url: format!(
            "wss://{HOST}/?api_key={}&access_token={}",
            encode(&credentials.0),
            encode(&credentials.1)
        ),
        host: HOST.to_owned(),
        port: 443,
        subscribe: vec![
            json!({"a": "subscribe", "v": tokens}).to_string(),
            json!({"a": "mode", "v": ["full", tokens]}).to_string(),
        ],
        // Kite sends one-byte binary heartbeats even while markets are closed.
        max_data_silence: Duration::from_secs(30),
        keepalive: Keepalive::Frame,
    }
}

fn india_day() -> i64 {
    (crate::model::watchlist::now_ms() + 19_800_000).div_euclid(86_400_000)
}

/// All lengths and byte order follow the official Kite protocol. A malformed frame
/// is rejected whole; unknown packet lengths are skipped for protocol extensions.
/// One-byte heartbeats and empty packet lists carry no quote.
fn decode(bytes: &[u8]) -> Option<Vec<(u32, SourceQuote)>> {
    if bytes.len() == 1 {
        return Some(Vec::new());
    }
    let count = usize::from(u16::from_be_bytes(bytes.get(..2)?.try_into().ok()?));
    if count > MAX_INSTRUMENTS {
        return None;
    }
    let mut offset = 2_usize;
    let mut ticks = Vec::with_capacity(count);
    for _ in 0..count {
        let length = usize::from(u16::from_be_bytes(
            bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
        ));
        offset = offset.checked_add(2)?;
        let end = offset.checked_add(length)?;
        let packet = bytes.get(offset..end)?;
        offset = end;
        if let Some(tick) = packet_quote(packet) {
            ticks.push(tick);
        }
    }
    (offset == bytes.len()).then_some(ticks)
}

fn packet_quote(packet: &[u8]) -> Option<(u32, SourceQuote)> {
    if !matches!(packet.len(), 8 | 28 | 32 | 44 | 184) {
        return None;
    }
    let token = number(packet, 0)?;
    if token == 0 {
        return None;
    }
    let segment = token & 0xff;
    // BSE currency derivatives (BCD) use a different divisor from NSE CDS.
    // See Zerodha's official pykiteconnect ticker implementation.
    let divisor = match segment {
        3 => 10_000_000.0,
        6 => 10_000.0,
        _ => 100.0,
    };
    let price = f64::from(number(packet, 4)?) / divisor;
    if price <= 0.0 {
        return None;
    }
    let mut quote = SourceQuote {
        price,
        ..SourceQuote::default()
    };
    match packet.len() {
        28 | 32 => {
            quote.previous_day_price = positive_price(packet, 20, divisor);
            if packet.len() == 32 {
                quote.timestamp_ms = timestamp(packet, 28);
            }
        }
        44 | 184 => {
            quote.previous_day_price = positive_price(packet, 40, divisor);
            if matches!(segment, 1 | 4) {
                quote.volume_24h = number(packet, 16).map(f64::from);
                quote.volume_kind = Some(VolumeKind::Shares);
            }
            if packet.len() == 184 {
                quote.timestamp_ms = timestamp(packet, 60).or_else(|| timestamp(packet, 44));
            }
        }
        _ => {}
    }
    Some((token, quote))
}

fn number(packet: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        packet
            .get(offset..offset.checked_add(4)?)?
            .try_into()
            .ok()?,
    ))
}

fn positive_price(packet: &[u8], offset: usize, divisor: f64) -> Option<f64> {
    number(packet, offset)
        .map(|price| f64::from(price) / divisor)
        .filter(|price| *price > 0.0)
}

fn timestamp(packet: &[u8], offset: usize) -> Option<i64> {
    number(packet, offset)
        .filter(|seconds| *seconds > 0)
        .map(|seconds| i64::from(seconds) * 1_000)
}

#[cfg(test)]
mod tests;
