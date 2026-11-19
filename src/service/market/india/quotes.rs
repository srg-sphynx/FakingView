use super::IndianProvider;
use super::catalog::{self, Instrument, Segment};
use crate::model::watchlist::quote::{SourceQuote, VolumeKind};
use crate::service::ProviderError;
use crate::service::market::request::{RateGate, encode};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

static IND_MONEY_QUOTES: RateGate = RateGate::new();
static GROWW_QUOTES: RateGate = RateGate::new();

pub fn quotes(
    provider: IndianProvider,
    symbols: &[String],
) -> Result<HashMap<String, SourceQuote>, ProviderError> {
    let token = provider.session()?;
    let directory = catalog::catalog(provider, &token)?;
    let resolved: Vec<_> = symbols
        .iter()
        .filter_map(|symbol| {
            catalog::find(&directory, symbol).map(|instrument| (symbol, instrument))
        })
        .collect();
    let mut quotes = HashMap::new();
    match provider {
        IndianProvider::IndMoney => {
            if !resolved.is_empty() {
                let requests: Vec<_> = resolved
                    .iter()
                    .map(|(symbol, instrument)| (*symbol, instrument.route.clone()))
                    .collect();
                let prices = fetch(provider, &token, Segment::Cash, &requests)?;
                quotes.extend(prices);
            }
        }
        IndianProvider::Groww => {
            for segment in [Segment::Cash, Segment::Derivatives] {
                let requests: Vec<_> = resolved
                    .iter()
                    .filter(|(_, instrument)| instrument.segment == segment)
                    .map(|(symbol, instrument)| (*symbol, groww_quote_id(instrument)))
                    .collect();
                if !requests.is_empty() {
                    quotes.extend(fetch(provider, &token, segment, &requests)?);
                }
            }
        }
    }
    Ok(quotes)
}

fn groww_quote_id(instrument: &Instrument) -> String {
    format!("{}_{}", instrument.exchange, instrument.ticker)
}

fn fetch(
    provider: IndianProvider,
    token: &str,
    segment: Segment,
    symbols: &[(&String, String)],
) -> Result<HashMap<String, SourceQuote>, ProviderError> {
    let routes = symbols
        .iter()
        .map(|(_, route)| encode(route))
        .collect::<Vec<_>>()
        .join(",");
    let value = match provider {
        IndianProvider::IndMoney => {
            IND_MONEY_QUOTES.wait(Duration::from_millis(210));
            provider.json(&format!("/market/quotes/full?scrip-codes={routes}"), token)?
        }
        IndianProvider::Groww => {
            GROWW_QUOTES.wait(Duration::from_millis(210));
            provider.json(
                &format!(
                    "/v1/live-data/ltp?segment={}&exchange_symbols={routes}",
                    segment.groww_name()
                ),
                token,
            )?
        }
    };
    parse(provider, &value, symbols)
}

fn parse(
    provider: IndianProvider,
    value: &Value,
    symbols: &[(&String, String)],
) -> Result<HashMap<String, SourceQuote>, ProviderError> {
    let payload = match (provider, value["status"].as_str()) {
        (IndianProvider::IndMoney, Some("success")) => &value["data"],
        (IndianProvider::Groww, Some("SUCCESS")) => &value["payload"],
        _ => return Err(provider.refused()),
    };
    let payload = payload
        .as_object()
        .ok_or_else(|| provider.data_error("returned invalid quote data"))?;
    Ok(symbols
        .iter()
        .filter_map(|(symbol, route)| {
            let row = payload.get(route)?;
            let quote = match provider {
                IndianProvider::IndMoney => SourceQuote {
                    price: positive(&row["live_price"])?,
                    previous_day_price: positive(&row["prev_close"]),
                    volume_24h: row["volume"].as_f64().filter(|volume| *volume >= 0.0),
                    volume_kind: Some(VolumeKind::Shares),
                    ..SourceQuote::default()
                },
                IndianProvider::Groww => SourceQuote {
                    price: positive(row)?,
                    ..SourceQuote::default()
                },
            };
            Some(((*symbol).clone(), quote))
        })
        .collect())
}

fn positive(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|price| price.is_finite() && *price > 0.0)
}

pub fn test_connection(provider: IndianProvider) -> Result<(), ProviderError> {
    let token = provider.session()?;
    let route = match provider {
        IndianProvider::IndMoney => "NSE_3045",
        IndianProvider::Groww => "NSE_RELIANCE",
    };
    let symbol = String::from("NSE");
    let quotes = fetch(
        provider,
        &token,
        Segment::Cash,
        &[(&symbol, route.to_owned())],
    )?;
    if !quotes.contains_key(&symbol) {
        return Err(provider.data_error("returned no price for the connection test"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn live_quotes_remap_routes_to_saved_ids_and_leave_absent_values_empty() {
        let symbol = String::from("NSE:INFY");
        let ind = json!({"status":"success","data":{"NSE_1594":{"live_price":125,"prev_close":120,"volume":200}}});
        let quotes = parse(
            IndianProvider::IndMoney,
            &ind,
            &[(&symbol, "NSE_1594".to_owned())],
        )
        .unwrap();
        assert_eq!(quotes[&symbol].price, 125.0);
        assert_eq!(quotes[&symbol].previous_day_price, Some(120.0));
        assert_eq!(quotes[&symbol].volume_24h, Some(200.0));

        let groww = json!({"status":"SUCCESS","payload":{"NSE_INFY":125,"BSE_INFY":null}});
        let bse = String::from("BSE:INFY");
        let quotes = parse(
            IndianProvider::Groww,
            &groww,
            &[
                (&symbol, "NSE_INFY".to_owned()),
                (&bse, "BSE_INFY".to_owned()),
            ],
        )
        .unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[&symbol].price, 125.0);
        assert_eq!(quotes[&symbol].previous_day_price, None);
    }
}
