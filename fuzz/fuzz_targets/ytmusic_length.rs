//! Reads any text as a song's length: never a panic, and a length read is the one its text writes
//! (docs/plugin-crates.md, section 4).

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_ytmusic::length::parse_length;

fuzz_target!(|text: &str| {
    let Some(length) = parse_length(text) else {
        return;
    };
    // Written back the way the service writes it, with as many fields, the length is the same text but for the
    // leading zeros of its first field.
    let seconds = length.as_secs();
    let written = match text.split(':').count() {
        2 => format!("{}:{:02}", seconds / 60, seconds % 60),
        3 => format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        ),
        fields => panic!("read {text:?}, which has {fields} fields"),
    };
    assert_eq!(
        text.trim_start_matches('0'),
        written.trim_start_matches('0'),
        "{text:?} read as {length:?}"
    );
});
