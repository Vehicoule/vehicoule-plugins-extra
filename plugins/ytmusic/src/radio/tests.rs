use super::*;
use crate::fixtures::*;
use serde_json::{Value, json};
use std::time::Duration;

/// A fake network: answers each request in turn from the answers it was given, records every request, and fails
/// the test on a request it has no answer for or one to another host.
struct FakeService {
    answers: Vec<Result<Response, Error>>,
    requests: Vec<Request>,
}

impl FakeService {
    fn new(answers: Vec<Result<Response, Error>>) -> Self {
        Self {
            answers,
            requests: Vec::new(),
        }
    }

    fn fetch(&mut self, request: Request) -> Result<Response, Error> {
        assert!(
            request.url.starts_with("https://music.youtube.com/"),
            "request to another host: {}",
            request.url
        );
        assert!(
            !self.answers.is_empty(),
            "unexpected request: {}",
            request.url
        );
        self.requests.push(request);
        self.answers.remove(0)
    }

    fn bodies(&self) -> Vec<Value> {
        self.requests
            .iter()
            .map(|request| serde_json::from_slice(&request.body).unwrap())
            .collect()
    }

    fn urls(&self) -> Vec<&str> {
        self.requests
            .iter()
            .map(|request| request.url.as_str())
            .collect()
    }
}

const SEARCH: &str = "https://music.youtube.com/youtubei/v1/search?prettyPrint=false";
const NEXT: &str = "https://music.youtube.com/youtubei/v1/next?prettyPrint=false";

fn seed() -> Wanted {
    Wanted {
        title: "Get Lucky (feat. Pharrell Williams & Nile Rodgers)".into(),
        artists: vec!["Daft Punk".into(), "Pharrell Williams".into()],
        length: Some(Duration::from_secs(369)),
        isrc: Some("USQX91300108".into()),
    }
}

fn daft_punk(length: &str) -> Vec<Value> {
    byline(
        &[("UCdp", "Daft Punk")],
        ("MPREram", "Random Access Memories"),
        length,
    )
}

fn search_with_the_seed() -> Response {
    ok(&search_answer(vec![
        search_item("radioedit", "Get Lucky (Radio Edit)", daft_punk("4:09")),
        search_item(
            "album",
            "Get Lucky (feat. Pharrell Williams and Nile Rodgers)",
            daft_punk("6:10"),
        ),
    ]))
}

fn queue(ids: &[&str]) -> Response {
    ok(&radio_answer(
        ids.iter()
            .map(|id| radio_item(id, &format!("Song {id}"), daft_punk("3:00"), "3:00"))
            .collect(),
    ))
}

fn ids(songs: &[Song]) -> Vec<&str> {
    songs.iter().map(|song| song.id.as_str()).collect()
}

#[test]
fn searches_the_seed_then_asks_for_the_radio_of_its_match() {
    let mut service = FakeService::new(vec![
        Ok(search_with_the_seed()),
        Ok(queue(&["album", "one", "two", "three"])),
    ]);
    let songs = continuation(&seed(), &[], 10, |request| service.fetch(request)).unwrap();
    assert_eq!(ids(&songs), ["one", "two", "three"]);
    assert_eq!(service.urls(), [SEARCH, NEXT]);
    let bodies = service.bodies();
    assert_eq!(bodies[0]["query"], json!("USQX91300108"));
    assert_eq!(bodies[1]["videoId"], json!("album"));
    assert_eq!(bodies[1]["playlistId"], json!("RDAMVMalbum"));
}

#[test]
fn searches_by_title_and_artist_without_an_isrc() {
    let mut service = FakeService::new(vec![Ok(search_with_the_seed()), Ok(queue(&["one"]))]);
    let mut seed = seed();
    seed.isrc = None;
    continuation(&seed, &[], 10, |request| service.fetch(request)).unwrap();
    assert_eq!(
        service.bodies()[0]["query"],
        json!("Get Lucky (feat. Pharrell Williams & Nile Rodgers) Daft Punk")
    );
}

#[test]
fn returns_at_most_count_songs() {
    let mut service = FakeService::new(vec![
        Ok(search_with_the_seed()),
        Ok(queue(&["album", "one", "two", "three"])),
    ]);
    let songs = continuation(&seed(), &[], 2, |request| service.fetch(request)).unwrap();
    assert_eq!(ids(&songs), ["one", "two"]);

    let mut service = FakeService::new(vec![
        Ok(search_with_the_seed()),
        Ok(queue(&["album", "one", "two", "three"])),
    ]);
    let songs = continuation(&seed(), &[], 1, |request| service.fetch(request)).unwrap();
    assert_eq!(ids(&songs), ["one"]);
}

#[test]
fn a_count_of_zero_sends_nothing() {
    let mut service = FakeService::new(Vec::new());
    assert_eq!(
        continuation(&seed(), &[], 0, |request| service.fetch(request)),
        Ok(Vec::new())
    );
    assert_eq!(service.requests.len(), 0);
}

#[test]
fn never_returns_the_seed_an_excluded_song_or_a_song_twice() {
    let mut service = FakeService::new(vec![
        Ok(search_with_the_seed()),
        Ok(queue(&[
            "album", "one", "excluded", "two", "one", "album", "other", "three",
        ])),
    ]);
    let exclude = [String::from("excluded"), String::from("other")];
    let songs = continuation(&seed(), &exclude, 10, |request| service.fetch(request)).unwrap();
    assert_eq!(ids(&songs), ["one", "two", "three"]);
}

#[test]
fn skipped_songs_do_not_count() {
    let mut service = FakeService::new(vec![
        Ok(search_with_the_seed()),
        Ok(queue(&["album", "excluded", "one", "one", "two", "three"])),
    ]);
    let songs = continuation(&seed(), &[String::from("excluded")], 2, |request| {
        service.fetch(request)
    })
    .unwrap();
    assert_eq!(ids(&songs), ["one", "two"]);
}

#[test]
fn a_seed_with_no_match_is_not_found_and_asks_for_no_radio() {
    let mut service = FakeService::new(vec![Ok(ok(&search_answer(vec![search_item(
        "x",
        "Another Song",
        daft_punk("6:10"),
    )])))]);
    assert_eq!(
        continuation(&seed(), &[], 10, |request| service.fetch(request)),
        Err(Error::NotFound)
    );
    assert_eq!(service.urls(), [SEARCH]);

    let mut service = FakeService::new(vec![Ok(ok(&search_answer_without_result()))]);
    assert_eq!(
        continuation(&seed(), &[], 10, |request| service.fetch(request)),
        Err(Error::NotFound)
    );
    assert_eq!(service.urls(), [SEARCH]);
}

#[test]
fn a_seed_that_cannot_be_searched_sends_nothing() {
    let mut service = FakeService::new(Vec::new());
    let mut seed = seed();
    seed.title = String::from("  ");
    assert!(matches!(
        continuation(&seed, &[], 10, |request| service.fetch(request)),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(service.requests.len(), 0);
}

#[test]
fn a_failed_search_is_returned_as_it_is_and_never_retried() {
    for failure in [
        Err(Error::Upstream(String::from("the request timed out"))),
        Ok(Response::new(
            429,
            vec![(String::from("Retry-After"), String::from("60"))],
            Vec::new(),
        )),
        Ok(Response::new(503, Vec::new(), Vec::new())),
        Ok(Response::new(200, Vec::new(), b"not json".to_vec())),
    ] {
        let expected = match &failure {
            Err(error) => error.clone(),
            Ok(response) => read_search(response).unwrap_err(),
        };
        let mut service = FakeService::new(vec![failure]);
        assert_eq!(
            continuation(&seed(), &[], 10, |request| service.fetch(request)),
            Err(expected)
        );
        assert_eq!(service.urls(), [SEARCH]);
    }
}

#[test]
fn a_failed_radio_is_returned_as_it_is_and_never_retried() {
    for failure in [
        Err(Error::Upstream(String::from(
            "the call's request budget is spent",
        ))),
        Ok(Response::new(429, Vec::new(), Vec::new())),
        Ok(Response::new(500, Vec::new(), Vec::new())),
        Ok(ok(&radio_answer_for_an_unknown_id())),
    ] {
        let expected = match &failure {
            Err(error) => error.clone(),
            Ok(response) => read_radio(response).unwrap_err(),
        };
        let mut service = FakeService::new(vec![Ok(search_with_the_seed()), failure]);
        assert_eq!(
            continuation(&seed(), &[], 10, |request| service.fetch(request)),
            Err(expected)
        );
        assert_eq!(service.urls(), [SEARCH, NEXT]);
    }
}
