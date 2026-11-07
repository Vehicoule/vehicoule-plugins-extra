//! The boundary: the contract's values in, the plugin's own values inside, the contract's values out.

use crate::bindings::vehicoule::plugin::http::{
    HttpError, Method, Request as HttpRequest, Response as HttpResponse,
};
use crate::bindings::vehicoule::plugin::types::{
    AlbumRef, ArtistRef, Artwork, Error as ContractError, Track,
};
use crate::model::{Error, Request, Response, Song, Wanted};
use crate::radio;
use std::time::Duration;

/// The `radio` call, from the contract's values to the contract's values, with the `fetch` it is given.
pub(crate) fn continuation(
    seed: &Track,
    exclude: &[String],
    count: u32,
    fetch: impl FnMut(Request) -> Result<Response, Error>,
) -> Result<Vec<Track>, ContractError> {
    radio::continuation(&wanted(seed), exclude, count, fetch)
        .and_then(|songs| songs.into_iter().map(track).collect())
        .map_err(contract_error)
}

fn wanted(track: &Track) -> Wanted {
    Wanted {
        title: track.title.clone(),
        artists: track
            .artists
            .iter()
            .map(|artist| artist.name.clone())
            .collect(),
        length: track.duration_ms.map(Duration::from_millis),
        isrc: track.isrc.clone(),
    }
}

fn track(song: Song) -> Result<Track, Error> {
    let duration_ms = song
        .length
        .map(|length| {
            u64::try_from(length.as_millis()).map_err(|_| {
                Error::Upstream(format!(
                    "a length of {} s does not fit in duration-ms",
                    length.as_secs()
                ))
            })
        })
        .transpose()?;
    Ok(Track {
        id: song.id,
        title: song.title,
        artists: song
            .artists
            .into_iter()
            .map(|artist| ArtistRef {
                id: artist.id,
                name: artist.name,
            })
            .collect(),
        album: song.album.map(|album| AlbumRef {
            id: album.id,
            title: album.name,
        }),
        duration_ms,
        // The service gives no ISRC.
        isrc: None,
        artwork: song
            .artwork
            .into_iter()
            .map(|thumbnail| Artwork {
                url: thumbnail.url,
                width: thumbnail.width,
                height: thumbnail.height,
            })
            .collect(),
    })
}

fn contract_error(error: Error) -> ContractError {
    match error {
        Error::NotFound => ContractError::NotFound,
        Error::RateLimited(delay) => {
            // A delay past `u64::MAX` milliseconds is half a billion years: saturating keeps its meaning.
            ContractError::RateLimited(
                delay.map(|delay| u64::try_from(delay.as_millis()).unwrap_or(u64::MAX)),
            )
        }
        Error::InvalidInput(message) => ContractError::InvalidInput(message),
        Error::Upstream(message) => ContractError::Upstream(message),
    }
}

pub(crate) fn to_http(request: Request) -> HttpRequest {
    HttpRequest {
        method: Method::Post,
        url: request.url,
        headers: request.headers,
        body: Some(request.body),
    }
}

pub(crate) fn from_http(result: Result<HttpResponse, HttpError>) -> Result<Response, Error> {
    match result {
        Ok(response) => Ok(Response::new(
            response.status,
            response.headers,
            response.body,
        )),
        Err(error) => Err(Error::Upstream(match error {
            HttpError::RefusedHost => String::from("the host refused the request's host"),
            HttpError::RefusedScheme => String::from("the host refused the request's scheme"),
            HttpError::BudgetExceeded => String::from("the call's request budget is spent"),
            HttpError::Timeout => String::from("the request timed out"),
            HttpError::Transport(message) => format!("the request failed: {message}"),
        })),
    }
}

#[cfg(test)]
mod tests;
