pub(super) const DAY_MS: i64 = 86_400_000;
pub(super) const IST_MS: i64 = 19_800_000;

pub(super) fn trading_date(timestamp: i64) -> i64 {
    timestamp.saturating_add(IST_MS).div_euclid(DAY_MS) * DAY_MS
}

/// Indian exchange timestamps may carry an explicit offset or use an implicit IST clock.
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
