//! Searches and matches any track `ytmusic` could be given (it comes from another plugin): never a panic, a search
//! text whenever the title can be searched, and a match only among the songs offered and never a rejected one
//! (docs/plugin-crates.md, section 4).

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::time::Duration;
use vehicoule_plugin_ytmusic::matching::{best_match, search_query};
use vehicoule_plugin_ytmusic::model::{Error, Link, Song, Wanted};

/// A song offered by the service: its id, title, artists' names and length in seconds.
type Offered = (String, String, Vec<String>, Option<u64>);

/// The track's title, artists, length in seconds and ISRC; the songs offered; the ids rejected.
type Input = (
    String,
    Vec<String>,
    Option<u64>,
    Option<String>,
    Vec<Offered>,
    Vec<String>,
);

fuzz_target!(|input: Input| {
    let (title, artists, length, isrc, offered, rejected) = input;
    let mut wanted = Wanted::default();
    wanted.title = title;
    wanted.artists = artists;
    wanted.length = length.map(Duration::from_secs);
    wanted.isrc = isrc;
    match search_query(&wanted) {
        Ok(query) => assert!(!query.trim().is_empty(), "{wanted:?}"),
        Err(Error::InvalidInput(_)) => assert!(
            !wanted.title.chars().any(char::is_alphanumeric),
            "{wanted:?}"
        ),
        Err(error) => panic!("{error:?} for {wanted:?}"),
    }
    let songs: Vec<Song> = offered
        .into_iter()
        .map(|(id, title, artists, length)| {
            let mut song = Song::default();
            song.id = id;
            song.title = title;
            song.artists = artists
                .into_iter()
                .map(|name| {
                    let mut link = Link::default();
                    link.name = name;
                    link
                })
                .collect();
            song.length = length.map(Duration::from_secs);
            song
        })
        .collect();
    if let Some(song) = best_match(&wanted, &songs, &rejected) {
        assert!(songs.iter().any(|offered| std::ptr::eq(offered, song)));
        assert!(
            !rejected.contains(&song.id),
            "matched the rejected {:?}",
            song.id
        );
        assert!(!song.artists.is_empty() && !wanted.artists.is_empty());
    }
});
