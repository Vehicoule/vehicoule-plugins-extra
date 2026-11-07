//! The readers: what the service answers, turned into songs or into the contract's error.
//!
//! Each reader is a fuzz target (docs/plugin-crates.md, section 4). A field the plugin relies on that is missing or of
//! the wrong type is an `upstream` error, never a default: the shape of each answer is the one the crate
//! documentation's "Service" section says was checked.

use crate::innertube::check_status;
use crate::length::parse_length;
use crate::model::{Error, Link, Response, Song, Thumbnail};
use serde_json::Value;

/// Where a search answer keeps its sections.
const SEARCH_SECTIONS: &str =
    "/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents";
/// Where a radio answer keeps its queue.
const RADIO_QUEUE: &str = "/contents/singleColumnMusicWatchNextResultsRenderer/tabbedRenderer/\
watchNextTabbedResultsRenderer/tabs/0/tabRenderer/content/musicQueueRenderer";
/// The page type of a link to an artist.
const ARTIST_PAGE: &str = "MUSIC_PAGE_TYPE_ARTIST";
/// The page type of a link to an album.
const ALBUM_PAGE: &str = "MUSIC_PAGE_TYPE_ALBUM";

/// Reads the songs of a search answer, in the service's order.
///
/// An answer with no song shelf, as the service gives when nothing matches, is an empty list.
///
/// # Errors
///
/// The status's error ([`check_status`]), or `upstream` for a body that is not JSON or not in the shape of a search
/// answer.
pub fn read_search(response: &Response) -> Result<Vec<Song>, Error> {
    let document = parse(response)?;
    let sections = array(field(&document, SEARCH_SECTIONS)?, SEARCH_SECTIONS)?;
    let mut songs = Vec::new();
    for section in sections {
        let Some(shelf) = section.get("musicShelfRenderer") else {
            continue;
        };
        for item in array(field(shelf, "/contents")?, "/contents")? {
            if let Some(item) = item.get("musicResponsiveListItemRenderer") {
                songs.push(search_song(item)?);
            }
        }
    }
    Ok(songs)
}

/// Reads the songs of a radio answer, in the service's order; the first is usually the song the radio started from.
///
/// # Errors
///
/// The status's error ([`check_status`]); `not-found` for a queue with no content, as the service answers for a video
/// id it does not know; `upstream` for a body that is not JSON or not in the shape of a radio answer.
pub fn read_radio(response: &Response) -> Result<Vec<Song>, Error> {
    let document = parse(response)?;
    let queue = field(&document, RADIO_QUEUE)?;
    let Some(content) = queue.get("content") else {
        return Err(Error::NotFound);
    };
    let items = array(
        field(content, "/playlistPanelRenderer/contents")?,
        "/playlistPanelRenderer/contents",
    )?;
    items
        .iter()
        .filter_map(|item| item.get("playlistPanelVideoRenderer"))
        .map(radio_song)
        .collect()
}

fn parse(response: &Response) -> Result<Value, Error> {
    check_status(response)?;
    serde_json::from_slice(&response.body)
        .map_err(|error| Error::Upstream(format!("the body is not JSON: {error}")))
}

fn search_song(item: &Value) -> Result<Song, Error> {
    let id = non_empty(
        string(field(item, "/playlistItemData/videoId")?, "videoId")?,
        "videoId",
    )?;
    let title = non_empty(
        text(field(
            item,
            "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text",
        )?)?,
        "title",
    )?;
    let byline = runs(field(
        item,
        "/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text",
    )?)?;
    let (artists, album) = links(byline)?;
    // The byline ends with the length, after the artists and the album: `Artist • Album • 6:10`.
    let mut length = None;
    for run in byline {
        if run.get("navigationEndpoint").is_none() {
            length = parse_length(string(field(run, "/text")?, "text")?).or(length);
        }
    }
    let artwork = thumbnails(field(
        item,
        "/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails",
    )?)?;
    Ok(Song {
        id,
        title,
        artists,
        album,
        length,
        artwork,
    })
}

fn radio_song(item: &Value) -> Result<Song, Error> {
    let id = non_empty(string(field(item, "/videoId")?, "videoId")?, "videoId")?;
    let title = non_empty(text(field(item, "/title")?)?, "title")?;
    let (artists, album) = links(runs(field(item, "/longBylineText")?)?)?;
    let length = match item.get("lengthText") {
        None => None,
        Some(length) => {
            let shown = text(length)?;
            Some(
                parse_length(&shown)
                    .ok_or_else(|| Error::Upstream(format!("{shown:?} is not a length")))?,
            )
        }
    };
    let artwork = thumbnails(field(item, "/thumbnail/thumbnails")?)?;
    Ok(Song {
        id,
        title,
        artists,
        album,
        length,
        artwork,
    })
}

/// The artists and the first album that a byline's runs link to; runs that link to nothing are separators or text.
fn links(runs: &[Value]) -> Result<(Vec<Link>, Option<Link>), Error> {
    let mut artists = Vec::new();
    let mut album = None;
    for run in runs {
        let Some(browse) = run.pointer("/navigationEndpoint/browseEndpoint") else {
            continue;
        };
        let page = browse
            .pointer(
                "/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType",
            )
            .and_then(Value::as_str);
        let link = || -> Result<Link, Error> {
            Ok(Link {
                id: non_empty(string(field(browse, "/browseId")?, "browseId")?, "browseId")?,
                name: string(field(run, "/text")?, "text")?.to_owned(),
            })
        };
        match page {
            Some(ARTIST_PAGE) => artists.push(link()?),
            Some(ALBUM_PAGE) if album.is_none() => album = Some(link()?),
            _ => {}
        }
    }
    Ok((artists, album))
}

fn thumbnails(list: &Value) -> Result<Vec<Thumbnail>, Error> {
    let mut artwork = Vec::new();
    for thumbnail in array(list, "thumbnails")? {
        let url = string(field(thumbnail, "/url")?, "url")?;
        // The host and the player only reach `https`; a thumbnail elsewhere is left out rather than handed on.
        if !url.starts_with("https://") {
            continue;
        }
        artwork.push(Thumbnail {
            url: url.to_owned(),
            width: pixels(thumbnail.get("width"), "width")?,
            height: pixels(thumbnail.get("height"), "height")?,
        });
    }
    Ok(artwork)
}

fn pixels(value: Option<&Value>, name: &str) -> Result<Option<u32>, Error> {
    let Some(value) = value else {
        return Ok(None);
    };
    let number = value
        .as_u64()
        .ok_or_else(|| Error::Upstream(format!("{name} is not a whole number")))?;
    u32::try_from(number).map(Some).map_err(|_| {
        Error::Upstream(format!(
            "{name} {number} does not fit in the contract's u32"
        ))
    })
}

/// The text of a `runs` object, its runs put together.
fn text(value: &Value) -> Result<String, Error> {
    runs(value)?
        .iter()
        .map(|run| string(field(run, "/text")?, "text"))
        .collect()
}

fn runs(value: &Value) -> Result<&[Value], Error> {
    array(field(value, "/runs")?, "runs")
}

fn field<'a>(value: &'a Value, pointer: &str) -> Result<&'a Value, Error> {
    value
        .pointer(pointer)
        .ok_or_else(|| Error::Upstream(format!("{pointer} is missing")))
}

fn array<'a>(value: &'a Value, name: &str) -> Result<&'a [Value], Error> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| Error::Upstream(format!("{name} is not a list")))
}

fn string<'a>(value: &'a Value, name: &str) -> Result<&'a str, Error> {
    value
        .as_str()
        .ok_or_else(|| Error::Upstream(format!("{name} is not a string")))
}

fn non_empty(text: impl Into<String>, name: &str) -> Result<String, Error> {
    let text = text.into();
    if text.is_empty() {
        Err(Error::Upstream(format!("{name} is empty")))
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests;
