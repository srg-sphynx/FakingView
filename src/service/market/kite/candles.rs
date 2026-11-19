//! Historical candles. Intraday aggregation uses minutes because Kite's larger bars begin at the
//! exchange session open, while FakingView's buckets begin at UTC midnight.

use super::{
    BASE_URL, DAY_MS, IST_MS, RateGate, credentials, encode, instruments, request, response_data,
};
use crate::model::market::bucket::{FoldPlan, bucket_start_ms};
use crate::model::{DataSource, Kline, Timeframe};
use crate::service::ProviderError;
use serde_json::Value;
use std::time::Duration;

const EARLIEST_MS: i64 = 946_684_800_000;
const MAX_BARS: usize = 30_000;
const MAX_PAGES: usize = 20;
static HISTORY: RateGate = RateGate::new();

pub(super) fn fetch_latest(
    symbol: &str,
    timeframe: Timeframe,
    limit: usize,
) -> Result<Vec<Kline>, ProviderError> {
    let plan = super::super::candles::plan(DataSource::Kite, timeframe)?;
    if limit == 0 {
        return Ok(Vec::new());
    }
    let requested = plan.base_count(limit).min(MAX_BARS);
    let now = crate::service::now_ms();
    let bars = fetch(symbol, plan.base(), EARLIEST_MS, now, requested)?;
    if bars.candles.is_empty() {
        return Err(ProviderError::other(
            "Kite Connect has no historical candles for this instrument. Check your data subscription and the instrument's availability.",
        ));
    }
    Ok(bars.shape(plan, limit))
}

pub(super) fn fetch_range(
    symbol: &str,
    timeframe: Timeframe,
    start_ms: i64,
    end_ms: i64,
    max: usize,
) -> Result<Vec<Kline>, ProviderError> {
    let plan = super::super::candles::plan(DataSource::Kite, timeframe)?;
    if max == 0 || start_ms >= end_ms {
        return Ok(Vec::new());
    }
    let requested = plan.base_count(max).min(MAX_BARS);
    let bars = fetch(
        symbol,
        plan.base(),
        start_ms.max(EARLIEST_MS),
        end_ms,
        requested,
    )?;
    Ok(bars
        .shape(plan, max)
        .into_iter()
        .filter(|bar| bar.open_time >= start_ms && bar.open_time < end_ms)
        .collect())
}

fn page_span(base: Timeframe, requested: usize) -> i64 {
    let days = if base == Timeframe::DAY_1 { 2_000 } else { 60 };
    // Weekends, holidays and short sessions leave large holes in wall time. Ask for a full week
    // even for a tiny tail refresh, then page further if that week has no session.
    let lookback = base
        .nominal_ms()
        .saturating_mul(requested as i64)
        .saturating_mul(if base.is_intraday() { 7 } else { 2 });
    lookback.clamp(7 * DAY_MS, days * DAY_MS)
}

struct History {
    candles: Vec<Kline>,
    /// Earliest instant covered by retained data, including known non-trading gaps.
    coverage_start: i64,
}

impl History {
    fn shape(self, plan: FoldPlan, limit: usize) -> Vec<Kline> {
        let truncated = self.candles.first().is_some_and(|first| {
            self.coverage_start > bucket_start_ms(plan.target(), first.open_time)
        });
        let mut bars = plan.fold(self.candles, false);
        // The generic fold preserves a sole forming bucket. A bounded historical
        // fetch whose coverage begins inside that bucket cannot establish its OHLC.
        if truncated && !plan.is_native() && !bars.is_empty() {
            bars.remove(0);
        }
        let skip = bars.len().saturating_sub(limit);
        bars.drain(..skip);
        bars
    }
}

fn fetch(
    symbol: &str,
    base: Timeframe,
    start_ms: i64,
    end_ms: i64,
    requested: usize,
) -> Result<History, ProviderError> {
    if start_ms >= end_ms || requested == 0 {
        return Ok(History {
            candles: Vec::new(),
            coverage_start: start_ms,
        });
    }
    let (key, access_token) = credentials()?;
    let token = instruments::token(symbol)?;
    let end = end_ms.min(crate::service::now_ms().saturating_add(DAY_MS));
    fetch_pages(base, start_ms, end, requested, |start, end| {
        HISTORY.wait(Duration::from_millis(350));
        let body = request(
            &history_url(token, base, start, end),
            &key,
            &access_token,
            16 * 1024 * 1024,
        )?;
        parse_candles(&body, base)
    })
}

fn fetch_pages(
    base: Timeframe,
    start_ms: i64,
    mut end: i64,
    requested: usize,
    mut page: impl FnMut(i64, i64) -> Result<Vec<Kline>, ProviderError>,
) -> Result<History, ProviderError> {
    let span = page_span(base, requested);
    let mut bars = Vec::new();
    let mut coverage_start = end;
    for _ in 0..MAX_PAGES {
        if end <= start_ms || bars.len() >= requested {
            break;
        }
        let start = end.saturating_sub(span).max(start_ms);
        let mut candles = page(start, end)?;
        // Enforce each half-open page locally so boundary duplicates cannot
        // prematurely fill the request or overwrite a newer page's tail.
        candles.retain(|bar| bar.open_time >= start && bar.open_time < end);
        bars.extend(candles);
        coverage_start = start;
        end = start;
    }
    let mut bars = super::super::candles::normalize(bars);
    let skip = bars.len().saturating_sub(requested);
    if skip > 0 {
        coverage_start = coverage_start.max(bars[skip - 1].close_time.saturating_add(1));
    }
    bars.drain(..skip);
    Ok(History {
        candles: bars,
        coverage_start,
    })
}

fn history_url(token: u32, base: Timeframe, start_ms: i64, end_ms: i64) -> String {
    let interval = if base == Timeframe::DAY_1 {
        "day"
    } else {
        "minute"
    };
    // Daily timestamps are represented as UTC labels of the Indian calendar date (see parser).
    // Convert those labels back to local midnight before formatting the API's IST query strings.
    let daily_adjust = if base == Timeframe::DAY_1 { IST_MS } else { 0 };
    format!(
        "{BASE_URL}/instruments/historical/{token}/{interval}?from={}&to={}&continuous=0&oi=0",
        encode(&ist_clock(start_ms.saturating_sub(daily_adjust))),
        encode(&ist_clock(
            end_ms.saturating_sub(daily_adjust).saturating_sub(1)
        ))
    )
}

fn ist_clock(epoch_ms: i64) -> String {
    let local = crate::format::iso8601(epoch_ms.saturating_add(IST_MS));
    local.get(..19).unwrap_or(&local).replace('T', " ")
}

fn parse_candles(body: &str, base: Timeframe) -> Result<Vec<Kline>, ProviderError> {
    let data = response_data(body)?;
    let rows = data
        .get("candles")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::data("Kite Connect response has no candle array"))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_array()
            .filter(|row| row.len() >= 6)
            .ok_or_else(|| ProviderError::data("Kite Connect returned an incomplete candle"))?;
        let timestamp = row[0].as_str().and_then(parse_timestamp).ok_or_else(|| {
            ProviderError::data("Kite Connect returned an invalid candle timestamp")
        })?;
        // IST midnight is on the previous UTC date. Preserve the Indian trading DATE as a UTC
        // calendar label so Monday, month and quarter folds do not attach it to Sunday/last month.
        let open_time = if base == Timeframe::DAY_1 {
            (timestamp + IST_MS).div_euclid(DAY_MS) * DAY_MS
        } else {
            timestamp
        };
        let numbers = row[1..6]
            .iter()
            .map(Value::as_f64)
            .collect::<Option<Vec<_>>>()
            .filter(|numbers| numbers.iter().all(|number| number.is_finite()))
            .ok_or_else(|| ProviderError::data("Kite Connect returned invalid candle numbers"))?;
        let [open, high, low, close, volume] = numbers[..] else {
            unreachable!("five candle fields");
        };
        if volume < 0.0
            || open < 0.0
            || high < 0.0
            || low < 0.0
            || close < 0.0
            || high < open.max(close)
            || low > open.min(close)
            || high < low
        {
            return Err(ProviderError::data(
                "Kite Connect returned an invalid candle range",
            ));
        }
        out.push(Kline {
            open_time,
            close_time: open_time + base.nominal_ms() - 1,
            open,
            high,
            low,
            close,
            volume,
        });
    }
    Ok(super::super::candles::normalize(out))
}

/// Kite historical timestamps include `+0530`; quote clocks omit it and are defined in IST.
/// Unlike the app's legacy UTC parser, this preserves explicit positive and negative offsets.
pub(super) fn parse_timestamp(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, time) = text.split_once(['T', ' '])?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    if date_parts.next().is_some()
        || !(1900..=2200).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
    {
        return None;
    }
    let days = crate::format::days_from_civil(year, month, day);
    if crate::format::civil_from_days(days) != (year, month, day) {
        return None;
    }
    let (clock, offset_ms) = if let Some(clock) = time.strip_suffix('Z') {
        (clock, 0)
    } else if let Some((index, sign)) = time
        .char_indices()
        .find(|(_, character)| matches!(character, '+' | '-'))
    {
        let raw_offset = &time[index + 1..];
        if raw_offset.len() != 4 && !(raw_offset.len() == 5 && raw_offset.as_bytes()[2] == b':') {
            return None;
        }
        let offset = raw_offset.replace(':', "");
        if offset.len() != 4 || !offset.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let hour: i64 = offset[..2].parse().ok()?;
        let minute: i64 = offset[2..].parse().ok()?;
        if hour > 14 || minute > 59 || hour == 14 && minute != 0 {
            return None;
        }
        (
            &time[..index],
            (hour * 60 + minute) * 60_000 * if sign == '+' { 1 } else { -1 },
        )
    } else {
        (time, IST_MS)
    };
    let mut parts = clock.split(':');
    let hour: i64 = parts.next()?.parse().ok()?;
    let minute: i64 = parts.next()?.parse().ok()?;
    let seconds = parts.next()?;
    if parts.next().is_some() || !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return None;
    }
    if seconds.ends_with('.') {
        return None;
    }
    let (second, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let second: i64 = second.parse().ok()?;
    if !(0..=59).contains(&second)
        || fraction.len() > 9
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let fraction = fraction
        .chars()
        .chain("000".chars())
        .take(3)
        .collect::<String>()
        .parse::<i64>()
        .ok()?;
    Some(days * DAY_MS + (hour * 3_600 + minute * 60 + second) * 1_000 + fraction - offset_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candle(time: &str) -> String {
        format!(r#"{{"status":"success","data":{{"candles":[["{time}",10,12,9,11,100]]}}}}"#)
    }
    fn date(year: i64, month: i64, day: i64) -> i64 {
        crate::format::days_from_civil(year, month, day) * DAY_MS
    }

    #[test]
    fn historical_and_quote_timestamps_preserve_the_ist_offset() {
        let utc = date(2026, 11, 19) + (3 * 60 + 45) * 60_000;
        for time in [
            "2026-11-19T09:15:00+0530",
            "2026-11-19T09:15:00+05:30",
            "2026-11-19 09:15:00",
            "2026-11-19T03:45:00Z",
            "2026-11-18T23:45:00-0400",
        ] {
            assert_eq!(parse_timestamp(time), Some(utc), "{time}");
        }
        assert_eq!(
            parse_timestamp("2026-11-19T09:15:00.123456+0530"),
            Some(utc + 123)
        );
    }

    #[test]
    fn impossible_dates_and_clocks_are_rejected() {
        for time in [
            "2026-02-29T09:15:00+0530",
            "2026-11-31T09:15:00+0530",
            "2026-11-19T24:00:00+0530",
            "2026-11-19T09:60:00+0530",
            "2026-11-19T09:15:00+0560",
            "2026-11-19T09:15:00oops",
            "99999-11-19T09:15:00Z",
        ] {
            assert_eq!(parse_timestamp(time), None, "{time}");
        }
        assert!(parse_timestamp("2024-02-29T00:00:00+0530").is_some());
    }

    #[test]
    fn indian_trading_dates_do_not_move_to_sunday_or_the_previous_month() {
        let bars = parse_candles(&candle("2026-06-01T00:00:00+0530"), Timeframe::DAY_1).unwrap();
        assert_eq!(bars[0].open_time, date(2026, 6, 1));
        assert_eq!(bars[0].close_time, date(2026, 6, 2) - 1);
        let weekly = crate::model::market::bucket::fold(&bars, Timeframe::WEEK_1, false);
        let monthly = crate::model::market::bucket::fold(&bars, Timeframe::MONTH_1, false);
        assert_eq!(weekly[0].open_time, date(2026, 6, 1));
        assert_eq!(monthly[0].open_time, date(2026, 6, 1));
    }

    #[test]
    fn minute_candles_keep_true_instant_and_non_native_hours_fold_on_utc_grid() {
        let bars = parse_candles(&candle("2026-11-19T09:15:00+0530"), Timeframe::MINUTE_1).unwrap();
        let time = date(2026, 11, 19) + 225 * 60_000;
        assert_eq!(bars[0].open_time, time);
        assert_eq!(bars[0].close_time, time + 59_999);
        let hourly = crate::model::market::bucket::fold(&bars, Timeframe::HOUR_1, false);
        assert_eq!(hourly[0].open_time, date(2026, 11, 19) + 3 * 3_600_000);
    }

    #[test]
    fn historical_urls_use_local_clocks_and_an_exclusive_upper_bound() {
        let start = date(2026, 11, 19) + 225 * 60_000;
        let url = history_url(256265, Timeframe::MINUTE_1, start, start + 60_000);
        assert!(url.contains(
            "/256265/minute?from=2026-11-19%2009%3A15%3A00&to=2026-11-19%2009%3A15%3A59"
        ));
        let day = history_url(
            256265,
            Timeframe::DAY_1,
            date(2026, 11, 19),
            date(2026, 11, 20),
        );
        assert!(day.contains("from=2026-11-19%2000%3A00%3A00&to=2026-11-19%2023%3A59%3A59"));
    }

    #[test]
    fn partial_or_malformed_candles_fail_explicitly() {
        for body in [
            "{}",
            r#"{"status":"success","data":{"candles":[["2026-11-19T00:00:00+0530",1]]}}"#,
            r#"{"status":"success","data":{"candles":[["bad",1,2,0,1,1]]}}"#,
            r#"{"status":"success","data":{"candles":[["2026-11-19T00:00:00+0530",1,2,0,1,-1]]}}"#,
        ] {
            assert!(parse_candles(body, Timeframe::DAY_1).is_err());
        }
        assert!(
            parse_candles(
                r#"{"status":"success","data":{"candles":[]}}"#,
                Timeframe::DAY_1
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn page_windows_obey_kite_limits_and_cover_long_weekends() {
        assert_eq!(page_span(Timeframe::MINUTE_1, 1), 7 * DAY_MS);
        assert_eq!(page_span(Timeframe::MINUTE_1, 30_000), 60 * DAY_MS);
        assert_eq!(page_span(Timeframe::DAY_1, 30_000), 2_000 * DAY_MS);
    }

    fn minute(time: i64) -> Kline {
        Kline {
            open_time: time,
            close_time: time + 59_999,
            open: 10.0,
            high: 12.0,
            low: 9.0,
            close: 11.0,
            volume: 1.0,
        }
    }

    #[test]
    fn page_limits_drop_a_cut_first_bucket_even_when_the_bar_count_is_not_full() {
        let end = date(2026, 11, 19) + 270 * 60_000;
        let mut calls = 0;
        let history = fetch_pages(
            Timeframe::MINUTE_1,
            end - 30 * 7 * DAY_MS,
            end,
            100,
            |start, _| {
                calls += 1;
                Ok(vec![
                    minute(start + 10 * 60_000),
                    minute(start + 70 * 60_000),
                ])
            },
        )
        .unwrap();
        assert_eq!(calls, MAX_PAGES);
        assert_eq!(history.candles.len(), MAX_PAGES * 2);
        let first = history.candles[0].open_time;
        let bars = history.shape(DataSource::Kite.plan(Timeframe::HOUR_1).unwrap(), 100);
        assert_eq!(bars.len(), MAX_PAGES * 2 - 1);
        assert!(bars[0].open_time > bucket_start_ms(Timeframe::HOUR_1, first));
    }

    #[test]
    fn a_session_open_gap_is_preserved_when_the_fetch_covers_the_bucket() {
        let start = date(2026, 11, 19);
        let end = start + 5 * 3_600_000;
        let history = fetch_pages(Timeframe::MINUTE_1, start, end, 100, |_, _| {
            Ok((225..300)
                .map(|minutes| minute(start + minutes * 60_000))
                .collect())
        })
        .unwrap();
        let bars = history.shape(DataSource::Kite.plan(Timeframe::HOUR_1).unwrap(), 100);
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].open_time, start + 3 * 3_600_000);
        assert_eq!(bars[0].volume, 15.0);
    }

    #[test]
    fn trimming_the_base_count_discards_only_the_incomplete_older_hour() {
        let start = date(2026, 11, 19) + 3 * 3_600_000;
        let history = fetch_pages(
            Timeframe::MINUTE_1,
            start,
            start + 2 * 3_600_000,
            61,
            |_, _| {
                Ok((0..120)
                    .map(|minutes| minute(start + minutes * 60_000))
                    .collect())
            },
        )
        .unwrap();
        let bars = history.shape(DataSource::Kite.plan(Timeframe::HOUR_1).unwrap(), 10);
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].open_time, start + 3_600_000);
        assert_eq!(bars[0].volume, 60.0);
    }

    #[test]
    fn a_sole_historical_fragment_is_not_presented_as_a_whole_candle() {
        let start = date(2026, 11, 19) + 3 * 3_600_000;
        let history = History {
            candles: vec![minute(start + 20 * 60_000)],
            coverage_start: start + 10 * 60_000,
        };
        assert!(
            history
                .shape(DataSource::Kite.plan(Timeframe::HOUR_1).unwrap(), 10)
                .is_empty()
        );
    }

    #[test]
    fn each_page_rejects_records_outside_its_exclusive_bounds() {
        let end = date(2026, 11, 19);
        let start = end - 14 * DAY_MS;
        let mut calls = 0;
        let history = fetch_pages(Timeframe::MINUTE_1, start, end, 3, |from, to| {
            calls += 1;
            Ok(vec![minute(from - 60_000), minute(from), minute(to)])
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(history.candles.len(), 2);
        assert_eq!(history.candles[0].open_time, start);
        assert_eq!(history.candles[1].open_time, start + 7 * DAY_MS);
    }

    #[test]
    fn invalid_high_low_bounds_are_rejected_instead_of_silently_repaired() {
        for numbers in [
            "10,9,8,10,1",
            "10,12,11,10,1",
            "10,12,8,13,1",
            "10,12,8,7,1",
        ] {
            let body = format!(
                r#"{{"status":"success","data":{{"candles":[["2026-11-19T00:00:00+0530",{numbers}]]}}}}"#
            );
            assert!(parse_candles(&body, Timeframe::DAY_1).is_err(), "{numbers}");
        }
    }

    #[test]
    fn malformed_timezone_punctuation_and_empty_fractional_seconds_are_rejected() {
        for text in [
            "2026-11-19T09:15:00+0:530",
            "2026-11-19T09:15:00+05::30",
            "2026-11-19T09:15:00.+0530",
        ] {
            assert!(parse_timestamp(text).is_none(), "{text}");
        }
    }
}
