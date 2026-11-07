//! The `radio` capability: songs that continue from a track, from the radio the service builds from one song.

use crate::innertube::{next_request, search_request};
use crate::matching::{best_match, search_query};
use crate::model::{Error, Request, Response, Song, Wanted};
use crate::read::{read_radio, read_search};

/// Up to `count` songs that continue from `seed`, in the service's order, none of them `seed` or in `exclude`.
///
/// Two requests: a search for the seed (it may come from another plugin, so its id means nothing here), then the
/// radio of the song that matches it. A failure of the first is returned as it is, and the second is not sent. A
/// count of zero sends nothing.
///
/// # Errors
///
/// `invalid-input` for a seed that cannot be searched, `not-found` when no song matches the seed or the service has
/// no radio for it, and every error of `fetch` and of the readers.
pub fn continuation(
    seed: &Wanted,
    exclude: &[String],
    count: u32,
    mut fetch: impl FnMut(Request) -> Result<Response, Error>,
) -> Result<Vec<Song>, Error> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let candidates = read_search(&fetch(search_request(&search_query(seed)?))?)?;
    let matched = best_match(seed, &candidates, &[]).ok_or(Error::NotFound)?;
    let queue = read_radio(&fetch(next_request(&matched.id))?)?;
    let limit = usize::try_from(count).unwrap_or(usize::MAX);
    let mut songs: Vec<Song> = Vec::new();
    for song in queue {
        if songs.len() == limit {
            break;
        }
        if song.id != matched.id
            && !exclude.contains(&song.id)
            && songs.iter().all(|kept| kept.id != song.id)
        {
            songs.push(song);
        }
    }
    Ok(songs)
}

#[cfg(test)]
mod tests;
