//! Read-only market data from the official INDstocks and Groww REST APIs.

mod catalog;
mod history;
mod quotes;

use crate::model::{DataSource, Kline, Timeframe};
use crate::service::keychain::market_tokens::MarketToken;
use crate::service::net::http;
use crate::service::{MarketSource, ProviderError, SearchResult};
use serde_json::Value;

pub(crate) use catalog::clear_catalogs;
pub use quotes::{quotes, test_connection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndianProvider {
    IndMoney,
    Groww,
}

pub const IND_MONEY: IndianProvider = IndianProvider::IndMoney;
pub const GROWW: IndianProvider = IndianProvider::Groww;

impl IndianProvider {
    pub fn source(self) -> DataSource {
        match self {
            Self::IndMoney => DataSource::IndMoney,
            Self::Groww => DataSource::Groww,
        }
    }

    pub fn token(self) -> MarketToken {
        match self {
            Self::IndMoney => MarketToken::IndMoney,
            Self::Groww => MarketToken::Groww,
        }
    }

    fn session(self) -> Result<String, ProviderError> {
        self.token()
            .read()
            .ok_or(ProviderError::MarketTokenMissing(self.source()))
    }

    fn base_url(self) -> &'static str {
        match self {
            Self::IndMoney => "https://api.indstocks.com",
            Self::Groww => "https://api.groww.in",
        }
    }

    fn request(self, path: &str, token: &str, limit: u64) -> Result<String, ProviderError> {
        let url = format!("{}{path}", self.base_url());
        let groww_auth;
        let headers: &[(&str, &str)] = match self {
            Self::IndMoney => &[("Authorization", token)],
            Self::Groww => {
                groww_auth = format!("Bearer {token}");
                &[
                    ("Authorization", &groww_auth),
                    ("Accept", "application/json"),
                    ("X-API-VERSION", "1.0"),
                ]
            }
        };
        http::get_text_with_limit(&url, headers, http::Retry::Never, limit).map_err(|error| {
            match error {
                http::Error::Status(401 | 403) => ProviderError::MarketTokenRefused(self.source()),
                error => ProviderError::MarketRequest {
                    source_name: self.source(),
                    error,
                },
            }
        })
    }

    fn json(self, path: &str, token: &str) -> Result<Value, ProviderError> {
        let body = self.request(path, token, 8 * 1024 * 1024)?;
        serde_json::from_str(&body).map_err(|_| self.data_error("returned invalid JSON"))
    }

    fn data_error(self, message: &str) -> ProviderError {
        ProviderError::data(format!("{} {message}", self.source().display_name()))
    }

    fn refused(self) -> ProviderError {
        ProviderError::other(format!(
            "{} refused the market-data request. Check your access token and API plan in Settings.",
            self.source().display_name()
        ))
    }
}

impl MarketSource for IndianProvider {
    fn fetch_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: u32,
    ) -> Result<Vec<Kline>, ProviderError> {
        history::latest(*self, symbol, timeframe, limit as usize)
    }

    fn fetch_klines_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_ms: i64,
        end_ms: i64,
        max: usize,
    ) -> Result<Vec<Kline>, ProviderError> {
        history::range(*self, symbol, timeframe, start_ms, end_ms, max)
    }

    fn search_tickers(&self, query: &str) -> Result<Vec<SearchResult>, ProviderError> {
        catalog::search(*self, query)
    }
}
