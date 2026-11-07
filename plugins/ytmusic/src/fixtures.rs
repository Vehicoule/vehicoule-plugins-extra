//! Synthetic answers in the service's shape, with made-up values (docs/plugin-crates.md, section 3).
//!
//! Each builder writes the part of an answer the crate documentation's "Service" section says was checked, and
//! nothing recorded from the service. Tests change one field of them to show what the readers do with it.

use crate::model::Response;
use serde_json::{Value, json};

/// A byline run that links an artist.
pub(crate) fn artist_run(id: &str, name: &str) -> Value {
    link_run(id, name, "MUSIC_PAGE_TYPE_ARTIST")
}

/// A byline run that links an album.
pub(crate) fn album_run(id: &str, title: &str) -> Value {
    link_run(id, title, "MUSIC_PAGE_TYPE_ALBUM")
}

fn link_run(id: &str, text: &str, page: &str) -> Value {
    json!({
        "text": text,
        "navigationEndpoint": {
            "clickTrackingParams": "made-up",
            "browseEndpoint": {
                "browseId": id,
                "browseEndpointContextSupportedConfigs": { "browseEndpointContextMusicConfig": { "pageType": page } },
            },
        },
    })
}

/// A byline run that links nothing: a separator, a length, a year.
pub(crate) fn text_run(text: &str) -> Value {
    json!({ "text": text })
}

/// The byline of a song: its artists, its album and a last plain run (`Artist • Album • 3:21`).
pub(crate) fn byline(artists: &[(&str, &str)], album: (&str, &str), last: &str) -> Vec<Value> {
    let mut runs = Vec::new();
    for (index, (id, name)) in artists.iter().enumerate() {
        if index > 0 {
            runs.push(text_run(" & "));
        }
        runs.push(artist_run(id, name));
    }
    runs.push(text_run(" • "));
    runs.push(album_run(album.0, album.1));
    runs.push(text_run(" • "));
    runs.push(text_run(last));
    runs
}

/// Thumbnails at the sizes the service gave, on its image host.
pub(crate) fn thumbnails(sizes: &[u32]) -> Value {
    Value::Array(
        sizes
            .iter()
            .map(|size| {
                json!({ "url": format!("https://yt3.googleusercontent.com/madeup=w{size}-h{size}"), "width": size, "height": size })
            })
            .collect(),
    )
}

/// A song of a search answer.
pub(crate) fn search_item(id: &str, title: &str, byline: Vec<Value>) -> Value {
    json!({
        "musicResponsiveListItemRenderer": {
            "trackingParams": "made-up",
            "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": thumbnails(&[60, 120]) } } },
            "flexColumns": [
                {
                    "musicResponsiveListItemFlexColumnRenderer": {
                        "text": { "runs": [{ "text": title, "navigationEndpoint": { "watchEndpoint": { "videoId": id } } }] },
                    },
                },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": Value::Array(byline) } } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "1.2M plays" }] } } },
            ],
            "playlistItemData": { "videoId": id },
        },
    })
}

/// A search answer whose song shelf holds these items, after the section the service puts first.
pub(crate) fn search_answer(items: Vec<Value>) -> Value {
    sections(vec![
        json!({ "itemSectionRenderer": { "contents": [{ "messageRenderer": { "text": { "runs": [{ "text": "About" }] } } }] } }),
        json!({ "musicShelfRenderer": { "title": { "runs": [{ "text": "Songs" }] }, "contents": Value::Array(items) } }),
    ])
}

/// A search answer with no result, as the service gives it.
pub(crate) fn search_answer_without_result() -> Value {
    sections(vec![json!({
        "itemSectionRenderer": {
            "contents": [{ "messageRenderer": { "text": { "runs": [{ "text": "No results for " }, { "text": "made up" }] } } }],
        },
    })])
}

fn sections(contents: Vec<Value>) -> Value {
    json!({
        "responseContext": { "visitorData": "made-up" },
        "contents": {
            "tabbedSearchResultsRenderer": {
                "tabs": [{ "tabRenderer": { "title": "YT Music", "content": { "sectionListRenderer": { "contents": Value::Array(contents) } } } }],
            },
        },
    })
}

/// A song of a radio answer.
pub(crate) fn radio_item(id: &str, title: &str, byline: Vec<Value>, length: &str) -> Value {
    json!({
        "playlistPanelVideoRenderer": {
            "videoId": id,
            "title": { "runs": [{ "text": title }] },
            "longBylineText": { "runs": Value::Array(byline) },
            "lengthText": { "runs": [{ "text": length }] },
            "thumbnail": { "thumbnails": thumbnails(&[60, 544]) },
            "navigationEndpoint": { "watchEndpoint": { "videoId": id, "playlistId": format!("RDAMVM{id}") } },
        },
    })
}

/// A radio answer whose queue holds these items.
pub(crate) fn radio_answer(items: Vec<Value>) -> Value {
    queue(
        json!({ "content": { "playlistPanelRenderer": { "contents": Value::Array(items), "isInfinite": true } } }),
    )
}

/// A radio answer for a video id the service does not know: a queue with no content.
pub(crate) fn radio_answer_for_an_unknown_id() -> Value {
    queue(
        json!({ "header": { "musicQueueHeaderRenderer": { "title": { "runs": [{ "text": "Playing from" }] } } } }),
    )
}

fn queue(mut renderer: Value) -> Value {
    json!({
        "responseContext": { "visitorData": "made-up" },
        "contents": {
            "singleColumnMusicWatchNextResultsRenderer": {
                "tabbedRenderer": {
                    "watchNextTabbedResultsRenderer": {
                        "tabs": [{ "tabRenderer": { "content": { "musicQueueRenderer": renderer.take() } } }],
                    },
                },
            },
        },
    })
}

/// A `200` answer with this JSON body.
pub(crate) fn ok(body: &Value) -> Response {
    Response::new(
        200,
        vec![(
            String::from("content-type"),
            String::from("application/json; charset=UTF-8"),
        )],
        body.to_string().into_bytes(),
    )
}
