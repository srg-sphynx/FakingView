use super::*;
use serde_json::Value;

fn put(packet: &mut [u8], offset: usize, number: u32) {
    packet[offset..offset + 4].copy_from_slice(&number.to_be_bytes());
}

fn packet(token: u32, length: usize, price: u32) -> Vec<u8> {
    let mut packet = vec![0_u8; length];
    put(&mut packet, 0, token);
    put(&mut packet, 4, price);
    packet
}

fn frame(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = (packets.len() as u16).to_be_bytes().to_vec();
    for packet in packets {
        bytes.extend_from_slice(&(packet.len() as u16).to_be_bytes());
        bytes.extend_from_slice(packet);
    }
    bytes
}

#[test]
fn full_mode_subscription_carries_only_requested_tokens_and_encodes_credentials() {
    let session = session(&("key+1".into(), "token&x=/".into()), &[408065, 884737]);
    assert_eq!(
        session.url,
        "wss://ws.kite.trade/?api_key=key%2B1&access_token=token%26x%3D%2F"
    );
    assert_eq!(session.host, HOST);
    assert_eq!(session.keepalive, Keepalive::Frame);
    let messages: Vec<Value> = session
        .subscribe
        .iter()
        .map(|message| serde_json::from_str(message).unwrap())
        .collect();
    assert_eq!(
        messages,
        [
            json!({"a":"subscribe","v":[408065,884737]}),
            json!({"a":"mode","v":["full",[408065,884737]]}),
        ]
    );
}

#[test]
fn decodes_every_instrument_in_one_frame_with_price_divisors() {
    let packets = [
        packet(408065, 8, 123_456),
        packet((21 << 8) | 3, 8, 823_456_789),
        packet((22 << 8) | 6, 8, 823_456),
    ];
    let decoded = decode(&frame(&packets)).unwrap();
    assert_eq!(decoded.len(), 3);
    assert_eq!(decoded[0].0, 408065);
    assert_eq!(decoded[0].1.price, 1234.56);
    assert!((decoded[1].1.price - 82.345_678_9).abs() < 1e-10);
    assert_eq!(decoded[2].1.price, 82.3456);
    for (_, quote) in decoded {
        assert!(quote.previous_day_price.is_none());
        assert!(quote.volume_24h.is_none());
        assert!(quote.timestamp_ms.is_none());
    }
}

#[test]
fn equity_full_quote_uses_previous_close_session_volume_and_exchange_timestamp() {
    let mut packet = packet(408065, 184, 123_456);
    put(&mut packet, 16, 27_100);
    put(&mut packet, 40, 120_000);
    put(&mut packet, 44, 1_794_999_990);
    put(&mut packet, 60, 1_795_000_000);
    let quote = decode(&frame(&[packet])).unwrap()[0].1;
    assert_eq!(quote.price, 1234.56);
    assert_eq!(quote.previous_day_price, Some(1200.0));
    assert_eq!(quote.volume_24h, Some(27_100.0));
    assert_eq!(quote.volume_kind, Some(VolumeKind::Shares));
    assert_eq!(quote.timestamp_ms, Some(1_795_000_000_000));
}

#[test]
fn quote_mode_does_not_read_timestamps_or_invent_a_previous_close() {
    let mut packet = packet(408065, 44, 123_456);
    put(&mut packet, 16, 0);
    let quote = decode(&frame(&[packet])).unwrap()[0].1;
    assert!(quote.previous_day_price.is_none());
    assert!(quote.timestamp_ms.is_none());
    assert_eq!(quote.volume_24h, Some(0.0));
}

#[test]
fn index_quotes_use_the_index_layout_and_have_no_volume() {
    for length in [28, 32] {
        let mut packet = packet((123 << 8) | 9, length, 2_410_025);
        put(&mut packet, 20, 2_400_000);
        // Index change at24 is deliberately not mistaken for volume/reference.
        put(&mut packet, 24, 10_025);
        if length == 32 {
            put(&mut packet, 28, 1_795_000_000);
        }
        let quote = decode(&frame(&[packet])).unwrap()[0].1;
        assert_eq!(quote.price, 24_100.25);
        assert_eq!(quote.previous_day_price, Some(24_000.0));
        assert!(quote.volume_24h.is_none());
        assert!(quote.volume_kind.is_none());
        assert_eq!(
            quote.timestamp_ms,
            (length == 32).then_some(1_795_000_000_000)
        );
    }
}

#[test]
fn derivative_volume_is_not_mislabelled_as_shares_or_quote_currency() {
    let mut packet = packet((21 << 8) | 2, 184, 123_456);
    put(&mut packet, 16, 50_000);
    put(&mut packet, 44, 1_795_000_000);
    let quote = decode(&frame(&[packet])).unwrap()[0].1;
    assert!(quote.volume_24h.is_none());
    assert!(quote.volume_kind.is_none());
    assert_eq!(quote.timestamp_ms, Some(1_795_000_000_000));
}

#[test]
fn heartbeats_empty_lists_and_unknown_packet_shapes_have_no_prices() {
    assert_eq!(decode(&[0]), Some(Vec::new()));
    assert_eq!(decode(&[0, 0]), Some(Vec::new()));
    assert_eq!(decode(&frame(&[vec![0; 12]])), Some(Vec::new()));
    assert_eq!(decode(&frame(&[packet(0, 8, 123)])), Some(Vec::new()));
    assert_eq!(decode(&frame(&[packet(408065, 8, 0)])), Some(Vec::new()));
}

#[test]
fn malformed_frames_are_rejected_without_partial_prices() {
    let good = frame(&[packet(408065, 44, 123_456), packet(884737, 8, 87_654)]);
    for length in 2..good.len() {
        assert!(decode(&good[..length]).is_none(), "length {length}");
    }
    assert!(decode(&[]).is_none());
    assert!(decode(&[0, 1]).is_none());
    assert!(decode(&[0, 1, 0]).is_none());
    assert!(decode(&[0, 1, 255, 255]).is_none());
    assert!(decode(&[255, 255]).is_none());
    let mut trailing = good;
    trailing.push(1);
    assert!(decode(&trailing).is_none());
}

#[test]
fn arbitrary_short_binary_input_never_panics() {
    let mut random = 0x1234_5678_u32;
    for length in 0..512 {
        let bytes: Vec<u8> = (0..length)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as u8
            })
            .collect();
        let _ = decode(&bytes);
    }
}
