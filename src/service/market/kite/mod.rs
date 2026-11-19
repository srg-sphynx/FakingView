//! Read-only Indian market data from Kite Connect. Secrets stay in the platform credential store.

mod candles;
mod instruments;
mod session;

use super::request::{RateGate, encode};
use crate::model::watchlist::quote::{SourceQuote, VolumeKind};
use crate::model::{DataSource, Kline, Timeframe};
use crate::service::net::http;
use crate::service::{MarketSource, ProviderError, SearchResult, rank_results};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub use session::{exchange_request_token, test_connection};

const BASE_URL: &str = "https://api.kite.trade";
use super::indian_time::{DAY_MS, IST_MS};

pub struct Kite;
pub const KITE: Kite = Kite;

fn credentials() -> Result<(String, String), ProviderError> {
    crate::service::keychain::api_keys::kite_credentials().ok_or(ProviderError::KiteMissingKey)
}

fn authorization(api_key: &str, access_token: &str) -> Result<String, ProviderError> {
    if !valid_secret(api_key) || !valid_secret(access_token) {
        return Err(ProviderError::KiteMissingKey);
    }
    Ok(format!("token {}:{}", api_key.trim(), access_token.trim()))
}

fn valid_secret(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value.len() <= 4096 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn request(url: &str, key: &str, access_token: &str, limit: u64) -> Result<String, ProviderError> {
    let auth = authorization(key, access_token)?;
    http::get_text_with_limit(
        url,
        &[("X-Kite-Version", "3"), ("Authorization", &auth)],
        http::Retry::Never,
        limit,
    )
    .map_err(map_http_error)
}

fn map_http_error(error: http::Error) -> ProviderError {
    match error {
        http::Error::Status(401 | 403) => ProviderError::KiteSessionExpired,
        error => ProviderError::Kite(error),
    }
}

/// Validate the common envelope without displaying an API response that could echo credentials.
fn response_data(body: &str) -> Result<Value, ProviderError> {
    let mut value: Value = serde_json::from_str(body)
        .map_err(|_| ProviderError::data("Kite Connect returned invalid JSON"))?;
    if value.get("status").and_then(Value::as_str) != Some("success") {
        return Err(match value.get("error_type").and_then(Value::as_str) {
            Some("TokenException") => ProviderError::KiteSessionExpired,
            Some("PermissionException") => ProviderError::other(
                "Kite Connect denied access. Check the app's market-data subscription and permissions.",
            ),
            Some("InputException") => {
                ProviderError::other("Kite Connect refused the request parameters.")
            }
            _ => ProviderError::other(
                "Kite Connect refused the request. Check your connection and session.",
            ),
        });
    }
    let data = value
        .get_mut("data")
        .ok_or_else(|| ProviderError::data("Kite Connect response has no data"))?;
    Ok(data.take())
}

impl MarketSource for Kite {
    fn fetch_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: u32,
    ) -> Result<Vec<Kline>, ProviderError> {
        candles::fetch_latest(symbol, timeframe, limit as usize)
    }

    fn fetch_klines_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_ms: i64,
        end_ms: i64,
        max: usize,
    ) -> Result<Vec<Kline>, ProviderError> {
        candles::fetch_range(symbol, timeframe, start_ms, end_ms, max)
    }

    fn search_tickers(&self, query: &str) -> Result<Vec<SearchResult>, ProviderError> {
        let instruments = instruments::catalog()?;
        let needle = query.trim().to_uppercase();
        let results = instruments
            .iter()
            .filter(|instrument| {
                instrument.symbol.contains(&needle)
                    || instrument.name.to_uppercase().contains(&needle)
            })
            .map(|instrument| {
                SearchResult::new(
                    DataSource::Kite,
                    format!(
                        "{}{}{}",
                        instrument.symbol,
                        crate::service::NAME_SEPARATOR,
                        instrument.name
                    ),
                    instrument.symbol.clone(),
                )
            })
            .collect();
        Ok(rank_results(results, query).into_iter().take(50).collect())
    }
}

pub(crate) fn clear_catalog() {
    instruments::clear();
}

/// Resolve current instrument tokens from stable exchange:symbol identities. Never persist tokens:
/// exchanges recycle derivative tokens after expiry.
pub(crate) fn instrument_tokens(symbols: &[String]) -> Result<Vec<(u32, String)>, ProviderError> {
    let catalog = instruments::catalog()?;
    let wanted: std::collections::HashSet<String> = symbols
        .iter()
        .map(|symbol| symbol.trim().to_uppercase())
        .collect();
    Ok(catalog
        .iter()
        .filter(|instrument| wanted.contains(&instrument.symbol))
        .map(|instrument| (instrument.token, instrument.symbol.clone()))
        .collect())
}

static QUOTES: RateGate = RateGate::new();

/// Snapshot prices, in INR, keyed by the caller's original symbol spelling.
pub(crate) fn quotes(symbols: &[String]) -> Result<HashMap<String, SourceQuote>, ProviderError> {
    if symbols.is_empty() {
        return Ok(HashMap::new());
    }
    if symbols.len() > 500 {
        return Err(ProviderError::other(
            "Kite Connect accepts at most 500 symbols per quote request.",
        ));
    }
    let (key, token) = credentials()?;
    let url = quotes_url(symbols);
    QUOTES.wait(Duration::from_millis(1_050));
    parse_quotes(&request(&url, &key, &token, 8 * 1024 * 1024)?, symbols)
}

fn quotes_url(symbols: &[String]) -> String {
    let query = symbols
        .iter()
        .map(|symbol| format!("i={}", encode(&symbol.trim().to_uppercase())))
        .collect::<Vec<_>>()
        .join("&");
    format!("{BASE_URL}/quote?{query}")
}

fn parse_quotes(
    body: &str,
    symbols: &[String],
) -> Result<HashMap<String, SourceQuote>, ProviderError> {
    let value = response_data(body)?;
    let data = value
        .as_object()
        .ok_or_else(|| ProviderError::data("Kite Connect quote data is not an object"))?;
    let mut quotes = HashMap::new();
    for symbol in symbols {
        let Some(quote) = data.get(&symbol.trim().to_uppercase()) else {
            continue;
        };
        let Some(price) = quote
            .get("last_price")
            .and_then(Value::as_f64)
            .filter(|price| price.is_finite() && *price > 0.0)
        else {
            continue;
        };
        let previous_day_price = quote
            .get("ohlc")
            .and_then(|ohlc| ohlc.get("close"))
            .and_then(Value::as_f64)
            .filter(|price| price.is_finite() && *price > 0.0);
        // The existing quote model has shares, but no contracts unit. Do not label derivatives
        // as shares, nor an index's absent volume as zero.
        let equity = symbol.trim().to_uppercase().starts_with("NSE:")
            || symbol.trim().to_uppercase().starts_with("BSE:");
        let index = quote
            .get("instrument_token")
            .and_then(Value::as_u64)
            .is_some_and(|token| token & 0xff == 9);
        let volume = equity
            .then(|| {
                quote
                    .get("volume")
                    .and_then(Value::as_f64)
                    .filter(|volume| volume.is_finite() && *volume >= 0.0)
            })
            .flatten()
            .filter(|_| !index);
        // The quote's exchange timestamp is local IST without an offset. Do not replace it with
        // download time: closed-session quotes retain their actual timestamp.
        let timestamp_ms = quote
            .get("last_trade_time")
            .and_then(Value::as_str)
            .and_then(candles::parse_timestamp)
            .or_else(|| {
                quote
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(candles::parse_timestamp)
            });
        quotes.insert(
            symbol.clone(),
            SourceQuote {
                price,
                previous_day_price,
                volume_24h: volume,
                volume_kind: volume.map(|_| VolumeKind::Shares),
                timestamp_ms,
            },
        );
    }
    Ok(quotes)
}

#[cfg(test)]
mod tests;
