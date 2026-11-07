//! Numbers the service writes as text, read by hand: a song's length (`6:10`, `1:02:03`) and the seconds of a
//! `Retry-After` header.
//!
//! The only code of the plugin that reads bytes itself, so it is proved with Kani beside its tests
//! (docs/plugin-crates.md, section 5).

use std::time::Duration;

/// Reads a song's length as the service shows it: `m:ss` or `h:mm:ss`.
///
/// Every field after the first has exactly two digits and is under 60; the first has at least one digit. Anything
/// else, including a length that does not fit in a `u64` of seconds, is `None`: the text is not a length, and no
/// length is made up for it.
///
/// ```
/// use std::time::Duration;
/// use vehicoule_plugin_ytmusic::length::parse_length;
///
/// assert_eq!(parse_length("6:10"), Some(Duration::from_secs(370)));
/// assert_eq!(parse_length("1:02:03"), Some(Duration::from_secs(3723)));
/// assert_eq!(parse_length("1.9B plays"), None);
/// ```
#[must_use]
pub fn parse_length(text: &str) -> Option<Duration> {
    parse_length_bytes(text.as_bytes())
}

fn parse_length_bytes(text: &[u8]) -> Option<Duration> {
    let mut seconds: u64 = 0;
    let mut fields: usize = 0;
    for field in text.split(|&byte| byte == b':') {
        let value = parse_digits(field)?;
        if fields > 0 && (field.len() != 2 || value >= 60) {
            return None;
        }
        seconds = seconds.checked_mul(60)?.checked_add(value)?;
        fields = fields.checked_add(1)?;
    }
    if (2..=3).contains(&fields) {
        Some(Duration::from_secs(seconds))
    } else {
        None
    }
}

/// Reads a non-empty run of ASCII digits as a `u64`; `None` for anything else, or past `u64::MAX`.
///
/// `str::parse` is not used because it also accepts a leading `+`.
pub(crate) fn parse_digits(text: &[u8]) -> Option<u64> {
    if text.is_empty() {
        return None;
    }
    let mut value: u64 = 0;
    for &byte in text {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(byte.checked_sub(b'0')?))?;
    }
    Some(value)
}

#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_minutes_and_seconds() {
        assert_eq!(parse_length("6:10"), Some(Duration::from_secs(370)));
        assert_eq!(parse_length("0:00"), Some(Duration::ZERO));
        assert_eq!(parse_length("59:59"), Some(Duration::from_secs(3599)));
        assert_eq!(parse_length("125:00"), Some(Duration::from_mins(125)));
    }

    #[test]
    fn reads_hours_minutes_and_seconds() {
        assert_eq!(parse_length("1:02:03"), Some(Duration::from_secs(3723)));
        assert_eq!(parse_length("10:00:00"), Some(Duration::from_secs(36000)));
    }

    #[test]
    fn refuses_what_is_not_a_length() {
        for text in [
            "",
            ":",
            "6",
            "6:",
            ":10",
            "6:1",
            "6:100",
            "6:60",
            "1:60:00",
            "1:00:60",
            "1:2:03",
            "1:02:03:04",
            "+6:10",
            "6:+1",
            "-6:10",
            " 6:10",
            "6:10 ",
            "6.10",
            "6:1a",
            "1.9B plays",
            "٣:١٠",
        ] {
            assert_eq!(parse_length(text), None, "{text:?}");
        }
    }

    #[test]
    fn refuses_a_length_past_u64_seconds() {
        // u64::MAX seconds is 18446744073709551615: one more minute does not fit.
        assert_eq!(
            parse_length("307445734561825860:15"),
            Some(Duration::from_secs(u64::MAX))
        );
        assert_eq!(parse_length("307445734561825860:16"), None);
        assert_eq!(parse_length("307445734561825861:00"), None);
        assert_eq!(parse_length("99999999999999999999:00"), None);
    }

    #[test]
    fn reads_digits_and_nothing_else() {
        assert_eq!(parse_digits(b"0"), Some(0));
        assert_eq!(parse_digits(b"120"), Some(120));
        assert_eq!(parse_digits(b"18446744073709551615"), Some(u64::MAX));
        assert_eq!(parse_digits(b"18446744073709551616"), None);
        assert_eq!(parse_digits(b""), None);
        assert_eq!(parse_digits(b"+1"), None);
        assert_eq!(parse_digits(b"1 "), None);
        assert_eq!(parse_digits(b"/"), None);
        assert_eq!(parse_digits(b":"), None);
    }
}
