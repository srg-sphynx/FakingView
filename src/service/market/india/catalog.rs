use super::IndianProvider;
use crate::service::market::indian_time::{DAY_MS, IST_MS};
use crate::service::market::instrument_csv::Records;
use crate::service::net::http;
use crate::service::{NAME_SEPARATOR, ProviderError, SearchResult, rank_results};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const MAX_CSV: u64 = 64 * 1024 * 1024;
const GROWW_CSV: &str = "https://growwapi-assets.groww.in/instruments/instrument.csv";

#[derive(Clone, Debug)]
pub(super) struct Instrument {
    pub exchange: String,
    pub ticker: String,
    pub name: String,
    pub route: String,
    pub segment: Segment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Segment {
    Cash,
    Derivatives,
}

impl Segment {
    pub fn groww_name(self) -> &'static str {
        match self {
            Self::Cash => "CASH",
            Self::Derivatives => "FNO",
        }
    }
}

type Directory = HashMap<String, Instrument>;
type Catalog = Mutex<Option<(i64, Arc<Directory>)>>;
static IND_MONEY: Catalog = Mutex::new(None);
static GROWW: Catalog = Mutex::new(None);

pub(crate) fn clear_catalogs() {
    *crate::lock::lock(&IND_MONEY) = None;
    *crate::lock::lock(&GROWW) = None;
}

pub(super) fn catalog(
    provider: IndianProvider,
    token: &str,
) -> Result<Arc<Directory>, ProviderError> {
    let slot = match provider {
        IndianProvider::IndMoney => &IND_MONEY,
        IndianProvider::Groww => &GROWW,
    };
    let day = (crate::service::now_ms() + IST_MS).div_euclid(DAY_MS);
    let mut held = crate::lock::lock(slot);
    if let Some((_, instruments)) = held.as_ref().filter(|(loaded, _)| *loaded == day) {
        return Ok(Arc::clone(instruments));
    }

    let mut instruments = Directory::new();
    match provider {
        IndianProvider::IndMoney => {
            for (source, segment) in [("equity", Segment::Cash), ("fno", Segment::Derivatives)] {
                let body = provider.request(
                    &format!("/market/instruments?source={source}"),
                    token,
                    MAX_CSV,
                )?;
                parse(provider, &body, Some(segment), &mut instruments)?;
            }
        }
        IndianProvider::Groww => {
            let body = http::get_text_with_limit(GROWW_CSV, &[], http::Retry::Transient, MAX_CSV)?;
            parse(provider, &body, None, &mut instruments)?;
        }
    }
    if instruments.is_empty() {
        return Err(provider.data_error("returned no supported instruments"));
    }
    let instruments = Arc::new(instruments);
    *held = Some((day, Arc::clone(&instruments)));
    Ok(instruments)
}

pub(super) fn search(
    provider: IndianProvider,
    query: &str,
) -> Result<Vec<SearchResult>, ProviderError> {
    let token = provider.session()?;
    let instruments = catalog(provider, &token)?;
    let needle = query.trim().to_uppercase();
    let results = instruments
        .iter()
        .filter(|(symbol, instrument)| {
            symbol.contains(&needle) || instrument.name.to_uppercase().contains(&needle)
        })
        .map(|(symbol, instrument)| {
            let label = if instrument.name.is_empty()
                || instrument.ticker.eq_ignore_ascii_case(&instrument.name)
            {
                symbol.clone()
            } else {
                format!("{symbol}{NAME_SEPARATOR}{}", instrument.name)
            };
            SearchResult::new(provider.source(), label, symbol.clone())
        })
        .collect();
    Ok(rank_results(results, query).into_iter().take(50).collect())
}

pub(super) fn find(instruments: &Directory, symbol: &str) -> Option<Instrument> {
    instruments.get(&symbol.trim().to_uppercase()).cloned()
}

fn parse(
    provider: IndianProvider,
    body: &str,
    ind_segment: Option<Segment>,
    instruments: &mut Directory,
) -> Result<(), ProviderError> {
    let mut rows = Records::new(body);
    let invalid = || provider.data_error("returned malformed instrument CSV");
    let headings = rows.next().map_err(|_| invalid())?.ok_or_else(invalid)?;
    let ([exchange, ticker, name, route], segment) = match provider {
        IndianProvider::IndMoney => (
            ["EXCH", "TRADING_SYMBOL", "CUSTOM_SYMBOL", "SECURITY_ID"],
            None,
        ),
        IndianProvider::Groww => (
            ["exchange", "trading_symbol", "name", "groww_symbol"],
            Some("segment"),
        ),
    };
    let index = |key: &str| {
        headings
            .iter()
            .position(|heading| heading.trim() == key)
            .ok_or_else(invalid)
    };
    let (exchange, ticker, name, route) = (
        index(exchange)?,
        index(ticker)?,
        index(name)?,
        index(route)?,
    );
    let segment = segment.map(index).transpose()?;

    while let Some(row) = rows.next().map_err(|_| invalid())? {
        if row.len() == 1 && row[0].trim().is_empty() {
            continue;
        }
        if row.len() != headings.len() {
            return Err(invalid());
        }
        let exchange = row[exchange].trim();
        let ticker = row[ticker].trim();
        let route = row[route].trim();
        let segment = match (ind_segment, segment.map(|index| row[index].trim())) {
            (Some(segment), _) => segment,
            (_, Some("CASH")) => Segment::Cash,
            (_, Some("FNO")) => Segment::Derivatives,
            _ => continue,
        };
        if !matches!(exchange, "NSE" | "BSE") || ticker.is_empty() || route.is_empty() {
            continue;
        }
        let route = match provider {
            IndianProvider::IndMoney => {
                let prefix = match (exchange, segment) {
                    (exchange, Segment::Cash) => exchange,
                    ("NSE", Segment::Derivatives) => "NFO",
                    _ => continue,
                };
                if route.parse::<u32>().ok().filter(|id| *id != 0).is_none() {
                    return Err(invalid());
                }
                format!("{prefix}_{route}")
            }
            IndianProvider::Groww => route.to_owned(),
        };
        let market = Instrument {
            exchange: exchange.into(),
            ticker: ticker.into(),
            name: row[name].trim().into(),
            route,
            segment,
        };
        let identity = format!("{exchange}:{ticker}").to_uppercase();
        // Catalogs can carry several cash series with one trading symbol. Keep their
        // first listing so chart and quote resolution pick the same entry.
        instruments.entry(identity).or_insert(market);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_preserves_exchange_and_api_specific_symbols() {
        let mut items = Directory::new();
        let csv = "exchange,segment,trading_symbol,groww_symbol,name\r\nNSE,CASH,INFY,NSE-INFY,\"Infosys, Ltd.\"\r\nBSE,CASH,INFY,BSE-INFY,Infosys\r\nNSE,FNO,ABC25DECFUT,NSE-ABC-24Dec25-FUT,\r\nMCX,COMMODITY,GOLD,MCX-GOLD,Gold\r\n";
        parse(IndianProvider::Groww, csv, None, &mut items).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(find(&items, "nse:infy").unwrap().name, "Infosys, Ltd.");
        assert_eq!(find(&items, "BSE:INFY").unwrap().route, "BSE-INFY");
        assert_eq!(
            find(&items, "NSE:ABC25DECFUT").unwrap().route,
            "NSE-ABC-24Dec25-FUT"
        );
        items.clear();
        let csv = "EXCH,SECURITY_ID,TRADING_SYMBOL,CUSTOM_SYMBOL\nNSE,51011,ABC25DECFUT,ABC Fut\n";
        parse(
            IndianProvider::IndMoney,
            csv,
            Some(Segment::Derivatives),
            &mut items,
        )
        .unwrap();
        assert_eq!(find(&items, "NSE:ABC25DECFUT").unwrap().route, "NFO_51011");
    }
}
