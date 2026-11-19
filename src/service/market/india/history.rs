use super::IndianProvider;
use super::catalog::{self, Instrument};
use crate::model::{Kline, Timeframe};
use crate::service::ProviderError;
use crate::service::market::indian_time::{DAY_MS, IST_MS, parse_timestamp, trading_date};
use crate::service::market::request::{RateGate, encode};
use crate::service::market::windowed_history::{History, paginate};
use serde_json::Value;
use std::time::Duration;

const MAX_BASE_CANDLES: usize = 30_000;
const MAX_PAGES: usize = 20;
static IND_MONEY_HISTORY: RateGate = RateGate::new();
static GROWW_HISTORY: RateGate = RateGate::new();

pub(super) fn latest(
    provider: IndianProvider,
    symbol: &str,
    timeframe: Timeframe,
    limit: usize,
) -> Result<Vec<Kline>, ProviderError> {
    let bars = range(
        provider,
        symbol,
        timeframe,
        0,
        crate::service::now_ms(),
        limit,
    )?;
    if bars.is_empty() && limit > 0 {
        return Err(ProviderError::other(format!(
            "{} has no historical candles for {symbol} in the available range.",
            provider.source().display_name()
        )));
    }
    Ok(bars)
}

pub(super) fn range(
    provider: IndianProvider,
    symbol: &str,
    timeframe: Timeframe,
    start_ms: i64,
    end_ms: i64,
    limit: usize,
) -> Result<Vec<Kline>, ProviderError> {
    let plan = crate::service::market::candles::plan(provider.source(), timeframe)?;
    let earliest = match provider {
        IndianProvider::IndMoney => 946_684_800_000, // 2000-01-01
        IndianProvider::Groww => 1_577_836_800_000,  // 2020-01-01
    };
    let start = start_ms.max(earliest);
    let end = end_ms.min(crate::service::now_ms().saturating_add(DAY_MS));
    if limit == 0 || start >= end {
        return Ok(Vec::new());
    }

    let token = provider.session()?;
    let instruments = catalog::catalog(provider, &token)?;
    let instrument = catalog::find(&instruments, symbol).ok_or_else(|| {
        ProviderError::other(format!(
            "{symbol} is absent from the current {} instrument directory.",
            provider.source().display_name()
        ))
    })?;
    let base = plan.base();
    let requested = plan.base_count(limit).min(MAX_BASE_CANDLES);
    let history = fetch(provider, &instrument, &token, base, start..end, requested)?;
    Ok(history.shape(plan, limit))
}

fn fetch(
    provider: IndianProvider,
    instrument: &Instrument,
    token: &str,
    base: Timeframe,
    bounds: std::ops::Range<i64>,
    requested: usize,
) -> Result<History, ProviderError> {
    let intraday = base == Timeframe::MINUTE_1;
    let (max_days, gate, spacing) = match provider {
        IndianProvider::IndMoney => (if intraday { 7 } else { 365 }, &IND_MONEY_HISTORY, 650),
        IndianProvider::Groww => (if intraday { 30 } else { 180 }, &GROWW_HISTORY, 350),
    };
    let span = base
        .nominal_ms()
        .saturating_mul(requested as i64)
        .saturating_mul(if intraday { 7 } else { 2 })
        .clamp(5 * DAY_MS, max_days * DAY_MS);
    paginate(
        span,
        MAX_PAGES,
        bounds.start,
        bounds.end,
        requested,
        |start, end| {
            gate.wait(Duration::from_millis(spacing));
            let path = history_path(provider, instrument, base, start, end);
            let value = provider.json(&path, token)?;
            parse(provider, &value, &instrument.route, base)
        },
    )
}

fn history_path(
    provider: IndianProvider,
    instrument: &Instrument,
    base: Timeframe,
    start: i64,
    end: i64,
) -> String {
    let daily = base == Timeframe::DAY_1;
    let interval = if daily { "1day" } else { "1minute" };
    // Daily chart labels represent the IST calendar date on the UTC grid. Translate
    // labels back to IST day boundaries before asking for that range.
    let shift = if daily { IST_MS } else { 0 };
    let (start, end) = (start - shift, end - shift);
    let route = encode(&instrument.route);
    match provider {
        IndianProvider::IndMoney => format!(
            "/market/historical/{interval}?scrip-codes={route}&start_time={start}&end_time={end}"
        ),
        IndianProvider::Groww => format!(
            "/v1/historical/candles?exchange={}&segment={}&groww_symbol={route}&start_time={}&end_time={}&candle_interval={interval}",
            instrument.exchange,
            instrument.segment.groww_name(),
            start.div_euclid(1_000),
            end.div_euclid(1_000)
        ),
    }
}

fn parse(
    provider: IndianProvider,
    value: &Value,
    route: &str,
    base: Timeframe,
) -> Result<Vec<Kline>, ProviderError> {
    let rows = match provider {
        IndianProvider::IndMoney => {
            if value["success"].as_bool() != Some(true) {
                return Err(provider.refused());
            }
            let data = value["data"]
                .as_object()
                .ok_or_else(|| provider.data_error("returned invalid historical data"))?;
            let Some(market) = data.get(route) else {
                // The API omits markets with no trading data in the window.
                return Ok(Vec::new());
            };
            &market["candles"]
        }
        IndianProvider::Groww => {
            if value["status"].as_str() != Some("SUCCESS") {
                return Err(provider.refused());
            }
            &value["payload"]["candles"]
        }
    };
    rows.as_array()
        .ok_or_else(|| provider.data_error("returned no candle array"))?
        .iter()
        .map(|row| candle(provider, row, base))
        .collect()
}

fn candle(provider: IndianProvider, row: &Value, base: Timeframe) -> Result<Kline, ProviderError> {
    let invalid = || provider.data_error("returned an invalid candle");
    let (timestamp, prices, volume) = match provider {
        IndianProvider::IndMoney => (
            row["ts"]
                .as_i64()
                .and_then(|seconds| seconds.checked_mul(1_000)),
            [&row["o"], &row["h"], &row["l"], &row["c"]],
            &row["v"],
        ),
        IndianProvider::Groww => (
            row[0].as_str().and_then(parse_timestamp),
            [&row[1], &row[2], &row[3], &row[4]],
            &row[5],
        ),
    };
    let timestamp = timestamp
        .filter(|time| *time > 0 && *time <= crate::service::now_ms() + DAY_MS)
        .ok_or_else(invalid)?;
    let open_time = if base == Timeframe::DAY_1 {
        trading_date(timestamp)
    } else {
        timestamp
    };
    let [Some(open), Some(high), Some(low), Some(close)] = prices.map(Value::as_f64) else {
        return Err(invalid());
    };
    let volume = if provider == IndianProvider::Groww && volume.is_null() {
        0.0 // Index candles do not report traded volume.
    } else {
        volume.as_f64().ok_or_else(invalid)?
    };
    if low < 0.0 || low > open.min(close) || high < open.max(close) || volume < 0.0 {
        return Err(invalid());
    }
    Ok(Kline {
        open_time,
        close_time: open_time + base.nominal_ms() - 1,
        open,
        high,
        low,
        close,
        volume,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn providers_use_distinct_candle_envelopes_but_the_same_indian_session_clock() {
        let bar = parse_timestamp("2025-09-24T09:15:00").unwrap();
        let ind = json!({"success":true,"data":{"NSE_1594":{"candles":[{"ts":bar / 1_000,"o":10,"h":12,"l":9,"c":11,"v":200}]}}});
        let groww = json!({"status":"SUCCESS","payload":{"candles":[["2025-09-24T09:15:00",10,12,9,11,200,null]]}});
        let ind = parse(
            IndianProvider::IndMoney,
            &ind,
            "NSE_1594",
            Timeframe::MINUTE_1,
        )
        .unwrap();
        let groww = parse(
            IndianProvider::Groww,
            &groww,
            "NSE-INFY",
            Timeframe::MINUTE_1,
        )
        .unwrap();
        assert_eq!(ind, groww);
        assert_eq!(ind[0].open_time, bar);
        assert_eq!(ind[0].close_time, bar + 59_999);
        let daily = json!({"status":"SUCCESS","payload":{"candles":[["2025-09-24T00:00:00",10,12,9,11,null,null]]}});
        let daily = parse(IndianProvider::Groww, &daily, "NSE-NIFTY", Timeframe::DAY_1).unwrap();
        assert_eq!(
            daily[0].open_time,
            parse_timestamp("2025-09-24T00:00:00Z").unwrap()
        );
    }

    #[test]
    fn empty_history_is_distinct_from_a_failed_or_malformed_response() {
        let parse = |value| {
            parse(
                IndianProvider::IndMoney,
                &value,
                "NSE_1594",
                Timeframe::MINUTE_1,
            )
        };
        assert!(parse(json!({"success":true,"data":{}})).unwrap().is_empty());
        assert!(parse(json!({"success":false,"data":{}})).is_err());
        assert!(parse(json!({"success":true,"data":{"NSE_1594":{"candles":[{}]}}})).is_err());
    }

    #[test]
    fn history_queries_encode_contract_names_and_keep_each_api_time_unit() {
        let instrument = Instrument {
            exchange: "NSE".to_owned(),
            ticker: "M&M".to_owned(),
            route: "NSE-M&M".to_owned(),
            name: String::new(),
            segment: catalog::Segment::Cash,
        };
        assert_eq!(
            history_path(
                IndianProvider::Groww,
                &instrument,
                Timeframe::MINUTE_1,
                1_500_000_000_000,
                1_500_000_060_000
            ),
            "/v1/historical/candles?exchange=NSE&segment=CASH&groww_symbol=NSE-M%26M&start_time=1500000000&end_time=1500000060&candle_interval=1minute"
        );
        let instrument = Instrument {
            route: "NSE_3045".to_owned(),
            ..instrument
        };
        assert_eq!(
            history_path(
                IndianProvider::IndMoney,
                &instrument,
                Timeframe::MINUTE_1,
                1_500_000_000_000,
                1_500_000_060_000
            ),
            "/market/historical/1minute?scrip-codes=NSE_3045&start_time=1500000000000&end_time=1500000060000"
        );
    }
}
