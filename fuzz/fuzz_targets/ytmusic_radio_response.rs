//! Reads any radio answer `ytmusic` could receive: never a panic, and every song it returns respects what the reader
//! is responsible for (docs/plugin-crates.md, section 4).

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_ytmusic::model::{Error, Response};
use vehicoule_plugin_ytmusic::read::read_radio;

fuzz_target!(|input: (u16, Vec<(String, String)>, &[u8])| {
    let (status, headers, body) = input;
    match read_radio(&Response::new(status, headers, body.to_vec())) {
        Ok(songs) => {
            assert_eq!(status, 200, "read an answer of status {status}");
            for song in songs {
                assert!(!song.id.is_empty() && !song.title.is_empty(), "{song:?}");
                assert!(
                    song.artists
                        .iter()
                        .chain(&song.album)
                        .all(|link| !link.id.is_empty()),
                    "{song:?}"
                );
                assert!(
                    song.artwork
                        .iter()
                        .all(|art| art.url.starts_with("https://")),
                    "{song:?}"
                );
            }
        }
        Err(Error::RateLimited(_)) => assert_eq!(status, 429),
        Err(Error::NotFound) => assert_eq!(status, 200),
        Err(Error::InvalidInput(_)) => panic!("a radio answer is never invalid input"),
        Err(_) => {}
    }
});
