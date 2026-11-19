//! Daily instrument directory. A token is an ephemeral routing address, never market identity.

use super::{BASE_URL, DAY_MS, IST_MS, credentials, request};
use crate::service::ProviderError;
use std::sync::{Arc, Mutex};

const MAX_COLUMNS: usize = 64;
const MAX_FIELD_BYTES: usize = 16 * 1024;
const MAX_RECORD_BYTES: usize = 128 * 1024;
const MAX_INSTRUMENTS: usize = 1_000_000;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Instrument {
    pub token: u32,
    pub symbol: String,
    pub name: String,
}

struct Loaded {
    day: i64,
    items: Arc<Vec<Instrument>>,
}

static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);
static REFRESH: Mutex<()> = Mutex::new(());

pub(super) fn clear() {
    let _refresh = crate::lock::lock(&REFRESH);
    *crate::lock::lock(&LOADED) = None;
}

fn held(day: i64) -> Option<Arc<Vec<Instrument>>> {
    crate::lock::lock(&LOADED)
        .as_ref()
        .filter(|loaded| loaded.day == day)
        .map(|loaded| Arc::clone(&loaded.items))
}

pub(super) fn catalog() -> Result<Arc<Vec<Instrument>>, ProviderError> {
    // Credentials are checked even on a warm cache. A disconnected session cannot silently search.
    let (key, token) = credentials()?;
    let day = (crate::service::now_ms() + IST_MS).div_euclid(DAY_MS);
    if let Some(items) = held(day) {
        return Ok(items);
    }
    let _refresh = crate::lock::lock(&REFRESH);
    if let Some(items) = held(day) {
        return Ok(items);
    }
    let body = request(
        &format!("{BASE_URL}/instruments"),
        &key,
        &token,
        64 * 1024 * 1024,
    )?;
    let items = Arc::new(parse(&body)?);
    if items.is_empty() {
        return Err(ProviderError::data("Kite Connect returned no instruments"));
    }
    // No stale fallback: yesterday's derivative token may now refer to a different contract.
    *crate::lock::lock(&LOADED) = Some(Loaded {
        day,
        items: Arc::clone(&items),
    });
    Ok(items)
}

pub(super) fn token(symbol: &str) -> Result<u32, ProviderError> {
    let symbol = symbol.trim().to_uppercase();
    catalog()?.iter().find(|item| item.symbol == symbol).map(|item| item.token)
        .ok_or_else(|| ProviderError::other("This instrument is not in Kite Connect's current directory. Use exchange:symbol, for example NSE:INFY."))
}

/// RFC 4180 records, including escaped quotes, CRLF and embedded newlines. Parsing records as
/// they arrive avoids a second copy of the entire (large) instrument directory.
struct Records<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Records<'a> {
    fn new(body: &'a str) -> Self {
        Self {
            bytes: body.trim_start_matches('\u{feff}').as_bytes(),
            offset: 0,
        }
    }

    fn next(&mut self) -> Result<Option<Vec<String>>, ProviderError> {
        if self.offset >= self.bytes.len() {
            return Ok(None);
        }
        let mut fields = Vec::new();
        let mut field = Vec::new();
        let start = self.offset;
        let mut quoted = false;
        let mut closed = false;
        while let Some(&byte) = self.bytes.get(self.offset) {
            self.offset += 1;
            if self.offset - start > MAX_RECORD_BYTES || field.len() > MAX_FIELD_BYTES {
                return Err(invalid_csv());
            }
            if quoted {
                if byte == b'"' {
                    if self.bytes.get(self.offset) == Some(&b'"') {
                        self.offset += 1;
                        field.push(b'"');
                    } else {
                        quoted = false;
                        closed = true;
                    }
                } else {
                    field.push(byte);
                }
                continue;
            }
            match byte {
                b'"' if field.is_empty() && !closed => quoted = true,
                b',' => {
                    push_field(&mut fields, std::mem::take(&mut field))?;
                    closed = false;
                }
                b'\n' | b'\r' => {
                    if byte == b'\r' && self.bytes.get(self.offset) == Some(&b'\n') {
                        self.offset += 1;
                    }
                    push_field(&mut fields, field)?;
                    return Ok(Some(fields));
                }
                b'"' => return Err(invalid_csv()),
                _ if closed => return Err(invalid_csv()),
                _ => field.push(byte),
            }
        }
        if quoted {
            return Err(invalid_csv());
        }
        push_field(&mut fields, field)?;
        Ok(Some(fields))
    }
}

fn invalid_csv() -> ProviderError {
    ProviderError::data("Kite Connect returned malformed instrument CSV")
}
fn text(bytes: Vec<u8>) -> Result<String, ProviderError> {
    String::from_utf8(bytes).map_err(|_| invalid_csv())
}

fn push_field(fields: &mut Vec<String>, field: Vec<u8>) -> Result<(), ProviderError> {
    if fields.len() >= MAX_COLUMNS || field.len() > MAX_FIELD_BYTES {
        return Err(invalid_csv());
    }
    fields.push(text(field)?);
    Ok(())
}

fn parse(body: &str) -> Result<Vec<Instrument>, ProviderError> {
    let mut records = Records::new(body);
    let header = records.next()?.ok_or_else(invalid_csv)?;
    if header
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != header.len()
    {
        return Err(invalid_csv());
    }
    let column = |name: &str| {
        header
            .iter()
            .position(|column| column == name)
            .ok_or_else(invalid_csv)
    };
    let token = column("instrument_token")?;
    let symbol = column("tradingsymbol")?;
    let name = column("name")?;
    let exchange = column("exchange")?;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while let Some(row) = records.next()? {
        if row.len() == 1 && row[0].is_empty() {
            continue;
        }
        if row.len() != header.len() {
            return Err(invalid_csv());
        }
        let Ok(instrument_token) = row[token].parse::<u32>() else {
            continue;
        };
        let market = row[exchange].trim();
        let ticker = row[symbol].trim();
        if instrument_token == 0
            || !matches!(
                market,
                "NSE" | "BSE" | "NFO" | "BFO" | "CDS" | "BCD" | "MCX" | "NSEIX"
            )
            || ticker.is_empty()
            || ticker.contains(':')
            || ticker.chars().any(char::is_control)
        {
            continue;
        }
        let symbol = format!("{market}:{ticker}");
        if !seen.insert(symbol.clone()) {
            return Err(ProviderError::data(
                "Kite Connect returned duplicate instrument identities",
            ));
        }
        if out.len() >= MAX_INSTRUMENTS {
            return Err(invalid_csv());
        }
        out.push(Instrument {
            token: instrument_token,
            symbol,
            name: if row[name].trim().is_empty() {
                ticker.to_owned()
            } else {
                row[name].trim().to_owned()
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_ids_include_the_exchange_and_csv_quotes_are_respected() {
        let csv = "instrument_token,tradingsymbol,name,exchange\r\n256265,INFY,\"Infosys, Ltd\",NSE\r\n42,INFY,\"Infosys \"\"B\"\"\",BSE\r\n7,NIFTY 50,,NSE\r\n";
        let rows = parse(csv).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            (
                rows[0].token,
                rows[0].symbol.as_str(),
                rows[0].name.as_str()
            ),
            (256265, "NSE:INFY", "Infosys, Ltd")
        );
        assert_eq!(rows[1].symbol, "BSE:INFY");
        assert_eq!(rows[1].name, "Infosys \"B\"");
        assert_eq!(rows[2].name, "NIFTY 50");
    }

    #[test]
    fn malformed_csv_and_changed_headers_fail_instead_of_becoming_an_empty_search() {
        for csv in [
            "status,message\nerror,secret",
            "instrument_token,tradingsymbol,name,exchange\n1,X,\"open,NSE",
            "instrument_token,tradingsymbol,name,exchange\n1,X,\"closed\"junk,NSE",
            "instrument_token,tradingsymbol,name,exchange\n1,X,NSE",
        ] {
            assert!(parse(csv).is_err(), "{csv}");
        }
        let bad_token =
            "instrument_token,tradingsymbol,name,exchange\n4294967296,X,X,NSE\n0,X,X,NSE\n";
        assert!(parse(bad_token).unwrap().is_empty());
    }

    #[test]
    fn records_support_bom_multiline_names_and_reordered_columns() {
        let rows = parse(
            "\u{feff}name,exchange,tradingsymbol,instrument_token\n\"Name\ncontinued\",NSE,X,1",
        )
        .unwrap();
        assert_eq!(rows[0].symbol, "NSE:X");
        assert_eq!(rows[0].name, "Name\ncontinued");
    }

    #[test]
    fn token_identity_does_not_collapse_same_symbol_on_different_exchanges() {
        let rows = parse(
            "instrument_token,tradingsymbol,name,exchange\n1,INFY,Infosys,NSE\n2,INFY,Infosys,BSE",
        )
        .unwrap();
        assert_ne!(rows[0].symbol, rows[1].symbol);
        assert!(
            parse("instrument_token,tradingsymbol,name,exchange\n1,X,X,NSE\n2,X,X,NSE").is_err()
        );
    }

    #[test]
    fn resource_caps_reject_oversized_fields_column_counts_and_records() {
        let huge_field = "x".repeat(MAX_FIELD_BYTES + 1);
        assert!(Records::new(&huge_field).next().is_err());
        let columns = std::iter::repeat_n("x", MAX_COLUMNS + 1)
            .collect::<Vec<_>>()
            .join(",");
        assert!(Records::new(&columns).next().is_err());
        let large_record = std::iter::repeat_n("x".repeat(MAX_FIELD_BYTES), 9)
            .collect::<Vec<_>>()
            .join(",");
        assert!(Records::new(&large_record).next().is_err());
    }

    #[test]
    fn duplicate_header_names_fail_instead_of_selecting_an_arbitrary_identity_column() {
        assert!(
            parse("instrument_token,tradingsymbol,name,exchange,exchange\n1,X,X,NSE,BSE").is_err()
        );
    }
}
