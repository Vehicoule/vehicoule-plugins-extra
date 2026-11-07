use super::*;
use crate::fixtures::*;
use serde_json::json;
use std::time::Duration;

fn get_lucky_byline() -> Vec<Value> {
    byline(
        &[("UCdp", "Daft Punk"), ("UCpw", "Pharrell Williams")],
        ("MPREram", "Random Access Memories"),
        "6:10",
    )
}

fn link(id: &str, name: &str) -> Link {
    Link {
        id: id.into(),
        name: name.into(),
    }
}

fn upstream(result: Result<Vec<Song>, Error>) -> String {
    match result {
        Err(Error::Upstream(message)) => message,
        other => panic!("expected an upstream error, got {other:?}"),
    }
}

/// Changes one field of a document at a JSON pointer.
fn with(mut document: Value, pointer: &str, value: Value) -> Value {
    *document.pointer_mut(pointer).unwrap() = value;
    document
}

/// Removes one field of a document at a JSON pointer.
fn without(mut document: Value, pointer: &str) -> Value {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    document
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key)
        .unwrap();
    document
}

const FIRST_SEARCH_ITEM: &str = "/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/1/musicShelfRenderer/contents/0/musicResponsiveListItemRenderer";
const FIRST_RADIO_ITEM: &str = "/contents/singleColumnMusicWatchNextResultsRenderer/tabbedRenderer/watchNextTabbedResultsRenderer/tabs/0/tabRenderer/content/musicQueueRenderer/content/playlistPanelRenderer/contents/0/playlistPanelVideoRenderer";

fn one_search_song() -> Value {
    search_answer(vec![search_item(
        "4D7u5KF7SP8",
        "Get Lucky (feat. Pharrell Williams)",
        get_lucky_byline(),
    )])
}

fn one_radio_song() -> Value {
    radio_answer(vec![radio_item(
        "iU7oF4OXZSE",
        "Lose Yourself to Dance",
        get_lucky_byline(),
        "5:54",
    )])
}

#[test]
fn reads_every_field_of_a_searched_song() {
    let songs = read_search(&ok(&one_search_song())).unwrap();
    assert_eq!(
        songs,
        vec![Song {
            id: "4D7u5KF7SP8".into(),
            title: "Get Lucky (feat. Pharrell Williams)".into(),
            artists: vec![link("UCdp", "Daft Punk"), link("UCpw", "Pharrell Williams")],
            album: Some(link("MPREram", "Random Access Memories")),
            length: Some(Duration::from_secs(370)),
            artwork: vec![
                Thumbnail {
                    url: "https://yt3.googleusercontent.com/madeup=w60-h60".into(),
                    width: Some(60),
                    height: Some(60)
                },
                Thumbnail {
                    url: "https://yt3.googleusercontent.com/madeup=w120-h120".into(),
                    width: Some(120),
                    height: Some(120)
                },
            ],
        }]
    );
}

#[test]
fn keeps_the_services_order() {
    let answer = search_answer(vec![
        search_item("b", "B", get_lucky_byline()),
        search_item("a", "A", get_lucky_byline()),
        search_item("c", "C", get_lucky_byline()),
    ]);
    let ids: Vec<String> = read_search(&ok(&answer))
        .unwrap()
        .into_iter()
        .map(|song| song.id)
        .collect();
    assert_eq!(ids, ["b", "a", "c"]);
}

#[test]
fn reads_songs_of_every_shelf_and_skips_other_items() {
    let mut answer = one_search_song();
    let sections = answer
        .pointer_mut("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .unwrap()
        .as_array_mut()
        .unwrap();
    sections.push(json!({ "musicShelfRenderer": { "contents": [
        { "musicMultiRowListItemRenderer": { "title": "made up" } },
        search_item("second", "Instant Crush", get_lucky_byline()),
    ] } }));
    let ids: Vec<String> = read_search(&ok(&answer))
        .unwrap()
        .into_iter()
        .map(|song| song.id)
        .collect();
    assert_eq!(ids, ["4D7u5KF7SP8", "second"]);
}

#[test]
fn a_search_without_result_is_empty() {
    assert_eq!(
        read_search(&ok(&search_answer_without_result())),
        Ok(Vec::new())
    );
    assert_eq!(read_search(&ok(&search_answer(Vec::new()))), Ok(Vec::new()));
}

#[test]
fn takes_the_last_plain_run_that_is_a_length() {
    let mut runs = get_lucky_byline();
    runs.push(text_run(" • "));
    runs.push(text_run("2013"));
    let answer = search_answer(vec![search_item("x", "Get Lucky", runs)]);
    assert_eq!(
        read_search(&ok(&answer)).unwrap()[0].length,
        Some(Duration::from_secs(370))
    );

    let mut runs = byline(&[("UCa", "Artist")], ("MPREb", "Album"), "3:01");
    runs.push(text_run(" • "));
    runs.push(text_run("4:02"));
    let answer = search_answer(vec![search_item("x", "Song", runs)]);
    assert_eq!(
        read_search(&ok(&answer)).unwrap()[0].length,
        Some(Duration::from_secs(242))
    );
}

#[test]
fn a_linked_run_is_never_read_as_a_length() {
    let runs = vec![
        artist_run("UCa", "3:01"),
        text_run(" • "),
        album_run("MPREb", "4:02"),
    ];
    let answer = search_answer(vec![search_item("x", "Song", runs)]);
    let song = &read_search(&ok(&answer)).unwrap()[0];
    assert_eq!(song.length, None);
    assert_eq!(song.artists, vec![link("UCa", "3:01")]);
    assert_eq!(song.album, Some(link("MPREb", "4:02")));
}

#[test]
fn a_song_without_a_length_has_none() {
    let runs = byline(&[("UCa", "Artist")], ("MPREb", "Album"), "1.2M plays");
    let answer = search_answer(vec![search_item("x", "Song", runs)]);
    assert_eq!(read_search(&ok(&answer)).unwrap()[0].length, None);
}

#[test]
fn keeps_the_first_album_and_every_artist_and_ignores_other_links() {
    let runs = vec![
        artist_run("UCa", "First"),
        album_run("MPREb1", "Album One"),
        link_to("UCx", "Somewhere", "MUSIC_PAGE_TYPE_USER_CHANNEL"),
        artist_run("UCb", "Second"),
        album_run("MPREb2", "Album Two"),
        json!({ "text": "watch", "navigationEndpoint": { "watchEndpoint": { "videoId": "v" } } }),
        json!({ "text": "no page", "navigationEndpoint": { "browseEndpoint": { "browseId": "UCz" } } }),
    ];
    let answer = search_answer(vec![search_item("x", "Song", runs)]);
    let song = &read_search(&ok(&answer)).unwrap()[0];
    assert_eq!(
        song.artists,
        vec![link("UCa", "First"), link("UCb", "Second")]
    );
    assert_eq!(song.album, Some(link("MPREb1", "Album One")));
}

fn link_to(id: &str, text: &str, page: &str) -> Value {
    json!({
        "text": text,
        "navigationEndpoint": { "browseEndpoint": {
            "browseId": id,
            "browseEndpointContextSupportedConfigs": { "browseEndpointContextMusicConfig": { "pageType": page } },
        } },
    })
}

#[test]
fn leaves_out_thumbnails_that_are_not_https_and_keeps_unknown_sizes_unknown() {
    let answer = with(
        one_search_song(),
        &format!("{FIRST_SEARCH_ITEM}/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"),
        json!([
            { "url": "http://yt3.googleusercontent.com/plain", "width": 60, "height": 60 },
            { "url": "//yt3.googleusercontent.com/relative" },
            { "url": "https://yt3.googleusercontent.com/kept" },
            { "url": "data:image/png;base64,AAAA", "width": 1, "height": 1 },
        ]),
    );
    let song = &read_search(&ok(&answer)).unwrap()[0];
    assert_eq!(
        song.artwork,
        vec![Thumbnail {
            url: "https://yt3.googleusercontent.com/kept".into(),
            width: None,
            height: None
        }]
    );
}

#[test]
fn a_size_past_the_contracts_u32_is_an_error() {
    let list = format!("{FIRST_SEARCH_ITEM}/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails");
    let answer = with(
        one_search_song(),
        &list,
        json!([{ "url": "https://a/b", "width": 4_294_967_295_u64, "height": 1 }]),
    );
    assert_eq!(
        read_search(&ok(&answer)).unwrap()[0].artwork[0].width,
        Some(u32::MAX)
    );
    for (width, height) in [
        (json!(4_294_967_296_u64), json!(1)),
        (json!(1), json!(4_294_967_296_u64)),
    ] {
        let answer = with(
            one_search_song(),
            &list,
            json!([{ "url": "https://a/b", "width": width, "height": height }]),
        );
        assert!(upstream(read_search(&ok(&answer))).contains("does not fit in the contract's u32"));
    }
}

#[test]
fn a_size_that_is_not_a_whole_number_is_an_error() {
    let list = format!("{FIRST_SEARCH_ITEM}/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails");
    for size in [json!(-1), json!(1.5), json!("60"), json!(null), json!([60])] {
        let answer = with(
            one_search_song(),
            &list,
            json!([{ "url": "https://a/b", "width": size.clone() }]),
        );
        assert_eq!(
            upstream(read_search(&ok(&answer))),
            "width is not a whole number",
            "{size}"
        );
        let answer = with(
            one_search_song(),
            &list,
            json!([{ "url": "https://a/b", "height": size.clone() }]),
        );
        assert_eq!(
            upstream(read_search(&ok(&answer))),
            "height is not a whole number",
            "{size}"
        );
    }
}

#[test]
fn a_missing_field_of_a_searched_song_is_an_error() {
    for pointer in [
        "/playlistItemData/videoId",
        "/playlistItemData",
        "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text",
        "/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text",
        "/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails",
        "/thumbnail",
    ] {
        let answer = without(one_search_song(), &format!("{FIRST_SEARCH_ITEM}{pointer}"));
        assert!(
            upstream(read_search(&ok(&answer))).ends_with("is missing"),
            "{pointer}"
        );
    }
    let answer = with(
        one_search_song(),
        &format!("{FIRST_SEARCH_ITEM}/flexColumns"),
        json!([]),
    );
    assert!(upstream(read_search(&ok(&answer))).ends_with("is missing"));
}

#[test]
fn a_field_of_the_wrong_type_in_a_searched_song_is_an_error() {
    for (pointer, value, message) in [
        (
            "/playlistItemData/videoId",
            json!(7),
            "videoId is not a string",
        ),
        ("/playlistItemData/videoId", json!(""), "videoId is empty"),
        (
            "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs",
            json!({}),
            "runs is not a list",
        ),
        (
            "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs",
            json!([]),
            "title is empty",
        ),
        (
            "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text",
            json!(1),
            "text is not a string",
        ),
        (
            "/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs",
            json!("a"),
            "runs is not a list",
        ),
        (
            "/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/6/text",
            json!(null),
            "text is not a string",
        ),
        (
            "/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text",
            json!(false),
            "text is not a string",
        ),
        (
            "/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId",
            json!(""),
            "browseId is empty",
        ),
        (
            "/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId",
            json!(1),
            "browseId is not a string",
        ),
        (
            "/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails",
            json!({}),
            "thumbnails is not a list",
        ),
        (
            "/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails/0/url",
            json!(5),
            "url is not a string",
        ),
    ] {
        let answer = with(
            one_search_song(),
            &format!("{FIRST_SEARCH_ITEM}{pointer}"),
            value,
        );
        assert_eq!(upstream(read_search(&ok(&answer))), message, "{pointer}");
    }
    let answer = without(
        one_search_song(),
        &format!(
            "{FIRST_SEARCH_ITEM}/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId"
        ),
    );
    assert_eq!(upstream(read_search(&ok(&answer))), "/browseId is missing");
    let answer = without(
        one_search_song(),
        &format!(
            "{FIRST_SEARCH_ITEM}/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/6/text"
        ),
    );
    assert_eq!(upstream(read_search(&ok(&answer))), "/text is missing");
}

#[test]
fn a_search_answer_in_another_shape_is_an_error() {
    let sections = "/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents";
    for document in [
        json!({}),
        json!([]),
        json!(null),
        json!({ "contents": {} }),
        without(
            one_search_song(),
            "/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer",
        ),
        with(
            one_search_song(),
            "/contents/tabbedSearchResultsRenderer/tabs",
            json!([]),
        ),
        radio_answer(Vec::new()),
    ] {
        assert_eq!(
            upstream(read_search(&ok(&document))),
            format!("{sections} is missing"),
            "{document}"
        );
    }
    assert_eq!(
        upstream(read_search(&ok(&with(
            one_search_song(),
            sections,
            json!({})
        )))),
        format!("{sections} is not a list")
    );
    let shelf = format!("{sections}/1/musicShelfRenderer");
    assert_eq!(
        upstream(read_search(&ok(&without(
            one_search_song(),
            &format!("{shelf}/contents")
        )))),
        "/contents is missing"
    );
    assert_eq!(
        upstream(read_search(&ok(&with(
            one_search_song(),
            &format!("{shelf}/contents"),
            json!(3)
        )))),
        "/contents is not a list"
    );
}

#[test]
fn a_body_that_is_not_json_is_an_error() {
    for body in [&b""[..], b"<html>", b"{", b"\xff\xfe", b"{\"contents\":}"] {
        let response = Response::new(200, Vec::new(), body.to_vec());
        assert!(
            upstream(read_search(&response)).starts_with("the body is not JSON: "),
            "{body:?}"
        );
        assert!(
            upstream(read_radio(&response)).starts_with("the body is not JSON: "),
            "{body:?}"
        );
    }
}

#[test]
fn a_nesting_past_serde_jsons_limit_is_an_error_not_a_crash() {
    let deep = "[".repeat(100_000);
    let response = Response::new(200, Vec::new(), deep.into_bytes());
    assert!(upstream(read_search(&response)).contains("recursion limit exceeded"));
    assert!(upstream(read_radio(&response)).contains("recursion limit exceeded"));
}

#[test]
fn an_error_status_is_the_contracts_error_whatever_the_body() {
    let body = one_search_song().to_string().into_bytes();
    let limited = Response::new(
        429,
        vec![(String::from("Retry-After"), String::from("30"))],
        body.clone(),
    );
    assert_eq!(
        read_search(&limited),
        Err(Error::RateLimited(Some(Duration::from_secs(30))))
    );
    assert_eq!(
        read_radio(&limited),
        Err(Error::RateLimited(Some(Duration::from_secs(30))))
    );
    let limited = Response::new(429, Vec::new(), Vec::new());
    assert_eq!(read_search(&limited), Err(Error::RateLimited(None)));
    for status in [400, 404, 500, 503] {
        let response = Response::new(status, Vec::new(), body.clone());
        assert_eq!(
            upstream(read_search(&response)),
            format!("the service answered status {status}")
        );
        assert_eq!(
            upstream(read_radio(&response)),
            format!("the service answered status {status}")
        );
    }
}

#[test]
fn reads_every_field_of_a_radio_song() {
    let songs = read_radio(&ok(&one_radio_song())).unwrap();
    assert_eq!(
        songs,
        vec![Song {
            id: "iU7oF4OXZSE".into(),
            title: "Lose Yourself to Dance".into(),
            artists: vec![link("UCdp", "Daft Punk"), link("UCpw", "Pharrell Williams")],
            album: Some(link("MPREram", "Random Access Memories")),
            length: Some(Duration::from_secs(354)),
            artwork: vec![
                Thumbnail {
                    url: "https://yt3.googleusercontent.com/madeup=w60-h60".into(),
                    width: Some(60),
                    height: Some(60)
                },
                Thumbnail {
                    url: "https://yt3.googleusercontent.com/madeup=w544-h544".into(),
                    width: Some(544),
                    height: Some(544)
                },
            ],
        }]
    );
}

#[test]
fn reads_radio_songs_in_order_and_skips_other_items() {
    let answer = radio_answer(vec![
        radio_item("seed", "Seed", get_lucky_byline(), "6:10"),
        json!({ "automixPreviewVideoRenderer": { "content": {} } }),
        radio_item("next", "Next", get_lucky_byline(), "3:00"),
    ]);
    let ids: Vec<String> = read_radio(&ok(&answer))
        .unwrap()
        .into_iter()
        .map(|song| song.id)
        .collect();
    assert_eq!(ids, ["seed", "next"]);
    assert_eq!(read_radio(&ok(&radio_answer(Vec::new()))), Ok(Vec::new()));
}

#[test]
fn a_radio_song_without_a_length_has_none_but_a_wrong_length_is_an_error() {
    let answer = without(one_radio_song(), &format!("{FIRST_RADIO_ITEM}/lengthText"));
    assert_eq!(read_radio(&ok(&answer)).unwrap()[0].length, None);
    let answer = with(
        one_radio_song(),
        &format!("{FIRST_RADIO_ITEM}/lengthText/runs/0/text"),
        json!("5 minutes"),
    );
    assert_eq!(
        upstream(read_radio(&ok(&answer))),
        "\"5 minutes\" is not a length"
    );
    let answer = with(
        one_radio_song(),
        &format!("{FIRST_RADIO_ITEM}/lengthText"),
        json!("5:54"),
    );
    assert_eq!(upstream(read_radio(&ok(&answer))), "/runs is missing");
    let answer = with(
        one_radio_song(),
        &format!("{FIRST_RADIO_ITEM}/lengthText/runs"),
        json!([{ "text": "5:" }, { "text": "54" }]),
    );
    assert_eq!(
        read_radio(&ok(&answer)).unwrap()[0].length,
        Some(Duration::from_secs(354))
    );
}

#[test]
fn an_unknown_seed_has_no_radio() {
    assert_eq!(
        read_radio(&ok(&radio_answer_for_an_unknown_id())),
        Err(Error::NotFound)
    );
}

#[test]
fn a_missing_field_of_a_radio_song_is_an_error() {
    for pointer in [
        "/videoId",
        "/title",
        "/longBylineText",
        "/thumbnail/thumbnails",
        "/thumbnail",
    ] {
        let answer = without(one_radio_song(), &format!("{FIRST_RADIO_ITEM}{pointer}"));
        assert!(
            upstream(read_radio(&ok(&answer))).ends_with("is missing"),
            "{pointer}"
        );
    }
}

#[test]
fn a_field_of_the_wrong_type_in_a_radio_song_is_an_error() {
    for (pointer, value, message) in [
        ("/videoId", json!(["x"]), "videoId is not a string"),
        ("/videoId", json!(""), "videoId is empty"),
        ("/title/runs", json!([]), "title is empty"),
        ("/title/runs/0/text", json!({}), "text is not a string"),
        ("/longBylineText/runs", json!(null), "runs is not a list"),
        (
            "/thumbnail/thumbnails",
            json!("x"),
            "thumbnails is not a list",
        ),
        (
            "/thumbnail/thumbnails/1/width",
            json!(-544),
            "width is not a whole number",
        ),
    ] {
        let answer = with(
            one_radio_song(),
            &format!("{FIRST_RADIO_ITEM}{pointer}"),
            value,
        );
        assert_eq!(upstream(read_radio(&ok(&answer))), message, "{pointer}");
    }
}

#[test]
fn a_radio_answer_in_another_shape_is_an_error() {
    let queue = "/contents/singleColumnMusicWatchNextResultsRenderer/tabbedRenderer/watchNextTabbedResultsRenderer/tabs/0/tabRenderer/content/musicQueueRenderer";
    for document in [
        json!({}),
        json!("text"),
        search_answer(Vec::new()),
        with(one_radio_song(), "/contents", json!([])),
    ] {
        assert_eq!(
            upstream(read_radio(&ok(&document))),
            format!("{queue} is missing"),
            "{document}"
        );
    }
    let panel = format!("{queue}/content/playlistPanelRenderer");
    assert_eq!(
        upstream(read_radio(&ok(&without(one_radio_song(), &panel)))),
        "/playlistPanelRenderer/contents is missing"
    );
    assert_eq!(
        upstream(read_radio(&ok(&with(
            one_radio_song(),
            &format!("{panel}/contents"),
            json!({})
        )))),
        "/playlistPanelRenderer/contents is not a list"
    );
}
