//! Kani proofs of the by-hand readers (docs/plugin-crates.md, section 5; ADR 0006).
//!
//! The bound is the length of the input, stated in each harness; it is never lowered.

use super::{parse_digits, parse_length_bytes};
use std::time::Duration;

/// The longest input the harnesses try: `hh:mm:ss` fits, and so does every shape around it.
const MAX_LEN: usize = 8;

/// No input up to `MAX_LEN` bytes makes the length reader panic or overflow.
#[kani::proof]
#[kani::unwind(10)]
fn length_never_panics() {
    let bytes: [u8; MAX_LEN] = kani::any();
    let len: usize = kani::any_where(|len| *len <= MAX_LEN);
    let _ = parse_length_bytes(&bytes[..len]);
}

/// `m:ss` reads as `m * 60 + ss` exactly when `ss` is under 60, for every digit `m` and every two digits `ss`.
#[kani::proof]
#[kani::unwind(10)]
fn length_reads_minutes_and_seconds() {
    let minutes: u8 = kani::any_where(|digit: &u8| *digit <= 9);
    let tens: u8 = kani::any_where(|digit: &u8| *digit <= 9);
    let units: u8 = kani::any_where(|digit: &u8| *digit <= 9);
    let text = [b'0' + minutes, b':', b'0' + tens, b'0' + units];
    let seconds = u64::from(tens) * 10 + u64::from(units);
    let expected = (seconds < 60).then(|| Duration::from_secs(u64::from(minutes) * 60 + seconds));
    assert_eq!(parse_length_bytes(&text), expected);
}

/// No input up to `MAX_LEN` bytes makes the digit reader panic, and what it reads is never more than the input's
/// digits allow.
#[kani::proof]
#[kani::unwind(10)]
fn digits_never_panic() {
    let bytes: [u8; MAX_LEN] = kani::any();
    let len: usize = kani::any_where(|len| *len <= MAX_LEN);
    if let Some(value) = parse_digits(&bytes[..len]) {
        assert!(len > 0);
        assert!(value < 100_000_000);
        assert!(bytes[..len].iter().all(u8::is_ascii_digit));
    }
}
