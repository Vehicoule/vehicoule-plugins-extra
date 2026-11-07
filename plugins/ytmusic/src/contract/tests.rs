use super::*;
use crate::fixtures::*;
use crate::model::{Link, Thumbnail};

fn seed() -> Track {
    Track {
        id: String::from("3135556"),
        title: String::from("Get Lucky"),
        artists: vec![ArtistRef {
            id: String::from("27"),
            name: String::from("Daft Punk"),
        }],
        album: None,
        duration_ms: Some(369_000),
        isrc: Some(String::from("USQX91300108")),
        artwork: Vec::new(),
    }
}

#[test]
fn a_track_from_another_plugin_is_looked_for_by_what_every_track_has() {
    let wanted = wanted(&seed());
    assert_eq!(wanted.title, "Get Lucky");
    assert_eq!(wanted.artists, ["Daft Punk"]);
    assert_eq!(wanted.length, Some(Duration::from_secs(369)));
    assert_eq!(wanted.isrc.as_deref(), Some("USQX91300108"));
    let mut bare = seed();
    bare.duration_ms = None;
    bare.isrc = None;
    bare.artists = Vec::new();
    let wanted = super::wanted(&bare);
    assert_eq!(
        (wanted.length, wanted.isrc, wanted.artists),
        (None, None, Vec::<String>::new())
    );
}

#[test]
fn a_song_becomes_the_contracts_track() {
    let song = Song {
        id: String::from("iU7oF4OXZSE"),
        title: String::from("Lose Yourself to Dance"),
        artists: vec![Link {
            id: String::from("UCdp"),
            name: String::from("Daft Punk"),
        }],
        album: Some(Link {
            id: String::from("MPREram"),
            name: String::from("Random Access Memories"),
        }),
        length: Some(Duration::from_millis(354_250)),
        artwork: vec![Thumbnail {
            url: String::from("https://yt3.googleusercontent.com/a"),
            width: Some(544),
            height: None,
        }],
    };
    let track = track(song).unwrap();
    assert_eq!(track.id, "iU7oF4OXZSE");
    assert_eq!(track.title, "Lose Yourself to Dance");
    assert_eq!(track.artists.len(), 1);
    assert_eq!(
        (track.artists[0].id.as_str(), track.artists[0].name.as_str()),
        ("UCdp", "Daft Punk")
    );
    let album = track.album.unwrap();
    assert_eq!(
        (album.id.as_str(), album.title.as_str()),
        ("MPREram", "Random Access Memories")
    );
    assert_eq!(track.duration_ms, Some(354_250));
    assert_eq!(track.isrc, None);
    assert_eq!(track.artwork.len(), 1);
    assert_eq!(
        (
            track.artwork[0].url.as_str(),
            track.artwork[0].width,
            track.artwork[0].height
        ),
        ("https://yt3.googleusercontent.com/a", Some(544), None)
    );
}

#[test]
fn a_song_without_a_length_or_an_album_has_none() {
    let track = track(Song {
        id: String::from("x"),
        title: String::from("X"),
        ..Song::default()
    })
    .unwrap();
    assert_eq!((track.duration_ms, track.album.is_none()), (None, true));
}

#[test]
fn a_length_past_the_contracts_milliseconds_is_an_error() {
    let longest = Duration::from_millis(u64::MAX);
    assert_eq!(
        track(Song {
            length: Some(longest),
            ..Song::default()
        })
        .unwrap()
        .duration_ms,
        Some(u64::MAX)
    );
    let too_long = Song {
        length: Some(longest + Duration::from_millis(1)),
        ..Song::default()
    };
    assert!(
        matches!(track(too_long), Err(Error::Upstream(message)) if message.ends_with("does not fit in duration-ms"))
    );
}

#[test]
fn every_error_is_the_contracts_error_of_the_same_name() {
    assert!(matches!(
        contract_error(Error::NotFound),
        ContractError::NotFound
    ));
    assert!(matches!(
        contract_error(Error::RateLimited(None)),
        ContractError::RateLimited(None)
    ));
    assert!(matches!(
        contract_error(Error::RateLimited(Some(Duration::from_secs(30)))),
        ContractError::RateLimited(Some(30_000))
    ));
    assert!(matches!(
        contract_error(Error::RateLimited(Some(Duration::from_secs(u64::MAX)))),
        ContractError::RateLimited(Some(u64::MAX))
    ));
    assert!(
        matches!(contract_error(Error::InvalidInput(String::from("a"))), ContractError::InvalidInput(m) if m == "a")
    );
    assert!(
        matches!(contract_error(Error::Upstream(String::from("b"))), ContractError::Upstream(m) if m == "b")
    );
}

#[test]
fn a_request_is_a_post_of_its_body() {
    let request = to_http(Request {
        url: String::from("https://music.youtube.com/x"),
        headers: vec![(String::from("a"), String::from("b"))],
        body: b"{}".to_vec(),
    });
    assert!(matches!(request.method, Method::Post));
    assert_eq!(request.url, "https://music.youtube.com/x");
    assert_eq!(request.headers, [(String::from("a"), String::from("b"))]);
    assert_eq!(request.body, Some(b"{}".to_vec()));
}

#[test]
fn a_response_is_kept_whole_and_every_http_error_is_upstream() {
    let response = HttpResponse {
        status: 503,
        headers: vec![(String::from("a"), String::from("b"))],
        body: b"x".to_vec(),
    };
    assert_eq!(
        from_http(Ok(response)),
        Ok(Response::new(
            503,
            vec![(String::from("a"), String::from("b"))],
            b"x".to_vec()
        ))
    );
    for (error, message) in [
        (
            HttpError::RefusedHost,
            "the host refused the request's host",
        ),
        (
            HttpError::RefusedScheme,
            "the host refused the request's scheme",
        ),
        (
            HttpError::BudgetExceeded,
            "the call's request budget is spent",
        ),
        (HttpError::Timeout, "the request timed out"),
        (
            HttpError::Transport(String::from("reset")),
            "the request failed: reset",
        ),
    ] {
        assert_eq!(
            from_http(Err(error)),
            Err(Error::Upstream(String::from(message)))
        );
    }
}

#[test]
fn a_whole_call_answers_with_the_contracts_values() {
    let search = ok(&search_answer(vec![search_item(
        "matched",
        "Get Lucky",
        byline(
            &[("UCdp", "Daft Punk")],
            ("MPREram", "Random Access Memories"),
            "6:10",
        ),
    )]));
    let radio = ok(&radio_answer(vec![
        radio_item(
            "matched",
            "Get Lucky",
            byline(&[("UCdp", "Daft Punk")], ("MPREram", "RAM"), "2013"),
            "6:10",
        ),
        radio_item(
            "next",
            "Instant Crush",
            byline(&[("UCdp", "Daft Punk")], ("MPREram", "RAM"), "2013"),
            "5:37",
        ),
    ]));
    let mut answers = vec![search, radio];
    let tracks = continuation(&seed(), &[], 5, |_| Ok(answers.remove(0))).unwrap();
    assert_eq!(answers.len(), 0);
    assert_eq!(tracks.len(), 1);
    assert_eq!(
        (tracks[0].id.as_str(), tracks[0].duration_ms),
        ("next", Some(337_000))
    );

    let failure = continuation(&seed(), &[], 5, |_| {
        Err(Error::Upstream(String::from("down")))
    });
    assert!(matches!(failure, Err(ContractError::Upstream(message)) if message == "down"));
}
