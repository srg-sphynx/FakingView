use super::*;

#[test]
fn credentials_are_not_put_into_urls_or_error_messages() {
    assert_eq!(authorization("key", "token").unwrap(), "token key:token");
    for (key, token) in [
        ("", "distinctive-credential-9127"),
        ("key", "bad\r\nAuthorization: x"),
        ("key", "two words"),
    ] {
        let error = authorization(key, token).unwrap_err();
        assert_eq!(error, ProviderError::KiteMissingKey);
        assert!(!error.to_string().contains(token));
    }
    let body =
        r#"{"status":"error","error_type":"InputException","message":"echoed-super-secret"}"#;
    assert!(
        !response_data(body)
            .unwrap_err()
            .to_string()
            .contains("echoed-super-secret")
    );
}

#[test]
fn auth_and_rate_limit_errors_remain_typed() {
    assert_eq!(
        map_http_error(http::Error::Status(403)),
        ProviderError::KiteSessionExpired
    );
    assert_eq!(
        map_http_error(http::Error::Status(401)),
        ProviderError::KiteSessionExpired
    );
    assert!(map_http_error(http::Error::Status(429)).is_rate_limited());
    assert!(!map_http_error(http::Error::Status(500)).is_rate_limited());
    assert_eq!(
        response_data(r#"{"status":"error","error_type":"TokenException"}"#).unwrap_err(),
        ProviderError::KiteSessionExpired
    );
    assert!(response_data(r#"{"data":{}}"#).is_err());
    assert!(response_data("not json").is_err());
}

#[test]
fn quote_queries_encode_exchange_ids_index_spaces_and_punctuation() {
    let url = quotes_url(&["nse:infy".into(), "NSE:NIFTY 50".into(), "NSE:M&M".into()]);
    assert_eq!(
        url,
        "https://api.kite.trade/quote?i=NSE%3AINFY&i=NSE%3ANIFTY%2050&i=NSE%3AM%26M"
    );
}

#[test]
fn quotes_use_previous_close_and_keep_missing_instruments_missing() {
    let body = r#"{"status":"success","data":{
        "NSE:INFY":{"last_price":1500.5,"ohlc":{"close":1490},"volume":4200,"last_trade_time":"2026-11-19 09:15:00"},
        "BSE:INFY":{"last_price":1501,"ohlc":{"close":1491}},
        "NSE:ZERO":{"last_price":0},
        "NSE:UNREQUESTED":{"last_price":55}
    }}"#;
    let symbols = [
        "nse:infy".into(),
        "BSE:INFY".into(),
        "NSE:ZERO".into(),
        "NSE:MISSING".into(),
    ];
    let quotes = parse_quotes(body, &symbols).unwrap();
    assert_eq!(quotes.len(), 2);
    let quote = quotes["nse:infy"];
    assert_eq!(
        (quote.price, quote.previous_day_price),
        (1500.5, Some(1490.0))
    );
    assert_eq!(
        (quote.volume_24h, quote.volume_kind),
        (Some(4200.0), Some(VolumeKind::Shares))
    );
    assert_eq!(
        quote.timestamp_ms,
        candles::parse_timestamp("2026-11-19T03:45:00Z")
    );
    assert_eq!(quotes["BSE:INFY"].price, 1501.0);
    assert!(quotes["BSE:INFY"].timestamp_ms.is_none());
    assert!(parse_quotes(r#"{"status":"success","data":[]}"#, &symbols).is_err());
}

#[test]
fn quote_timestamps_fall_back_to_the_exchange_clock_for_indices() {
    let body = r#"{"status":"success","data":{"NSE:NIFTY 50":{"last_price":25000,"last_trade_time":null,"timestamp":"2026-11-19 15:30:00"}}}"#;
    let quote = parse_quotes(body, &["NSE:NIFTY 50".into()]).unwrap()["NSE:NIFTY 50"];
    assert_eq!(
        quote.timestamp_ms,
        candles::parse_timestamp("2026-11-19 15:30:00")
    );
}

#[test]
fn kite_uses_minute_and_daily_bases_so_session_bars_cannot_shift_utc_buckets() {
    assert_eq!(
        DataSource::Kite.natives(),
        &[Timeframe::MINUTE_1, Timeframe::DAY_1]
    );
    for text in ["5m", "22m", "1h", "4h"] {
        assert_eq!(
            DataSource::Kite
                .plan(Timeframe::parse(text).unwrap())
                .unwrap()
                .base(),
            Timeframe::MINUTE_1
        );
    }
    for text in ["2D", "1W", "1M", "3M", "1Y"] {
        assert_eq!(
            DataSource::Kite
                .plan(Timeframe::parse(text).unwrap())
                .unwrap()
                .base(),
            Timeframe::DAY_1
        );
    }
}

/// Uses the real shared HTTP transport against loopback, without changing provider endpoints.
fn serve_once(status: u16, body: String) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/quote?i=NSE%3AINFY",
        listener.local_addr().unwrap()
    );
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut received = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !received.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0, "connection ended before request headers");
            received.extend_from_slice(&buffer[..count]);
        }
        write!(
            stream,
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        String::from_utf8(received).unwrap()
    });
    (url, worker)
}

#[test]
fn rest_transport_sends_authentication_headers_and_preserves_the_body() {
    let body = r#"{"status":"success","data":{}}"#;
    let (url, server) = serve_once(200, body.into());
    assert_eq!(request(&url, "key", "token", 1024).unwrap(), body);
    let sent = server.join().unwrap().to_lowercase();
    assert!(sent.starts_with("get /quote?i=nse%3ainfy http/1.1\r\n"));
    assert!(sent.contains("\r\nauthorization: token key:token\r\n"));
    assert!(sent.contains("\r\nx-kite-version: 3\r\n"));
    assert!(!sent.lines().next().unwrap().contains("token"));
}

#[test]
fn rest_transport_keeps_auth_rate_limits_and_body_caps_typed() {
    for status in [401, 403, 429] {
        let (url, server) = serve_once(status, "echoed-super-secret".into());
        let error = request(&url, "key", "token", 1024).unwrap_err();
        if status == 429 {
            assert!(error.is_rate_limited());
        } else {
            assert_eq!(error, ProviderError::KiteSessionExpired);
        }
        assert!(!error.to_string().contains("echoed-super-secret"));
        server.join().unwrap();
    }
    let (url, server) = serve_once(200, "x".repeat(100));
    assert!(matches!(
        request(&url, "key", "token", 8),
        Err(ProviderError::Kite(http::Error::BodyTooLarge(8)))
    ));
    server.join().unwrap();
}
