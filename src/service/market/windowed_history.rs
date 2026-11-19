use crate::model::Kline;
use crate::model::market::bucket::{FoldPlan, bucket_start_ms};
use crate::service::ProviderError;

pub(super) struct History {
    pub(super) candles: Vec<Kline>,
    /// Earliest instant covered by retained data, including known non-trading gaps.
    pub(super) coverage_start: i64,
}

impl History {
    pub(super) fn shape(self, plan: FoldPlan, limit: usize) -> Vec<Kline> {
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

pub(super) fn paginate(
    span: i64,
    max_pages: usize,
    start_ms: i64,
    mut end: i64,
    requested: usize,
    mut page: impl FnMut(i64, i64) -> Result<Vec<Kline>, ProviderError>,
) -> Result<History, ProviderError> {
    let mut bars = Vec::new();
    let mut coverage_start = end;
    for _ in 0..max_pages {
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
    let mut bars = super::candles::normalize(bars);
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
