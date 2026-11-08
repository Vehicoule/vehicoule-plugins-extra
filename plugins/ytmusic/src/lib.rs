//! `ytmusic`: radio candidates from `YouTube Music`.
//!
//! The plugin provides `radio` (Vehicoule's plugin providers, section 2): given a track from any plugin, it finds that
//! track on `YouTube Music` and returns the songs of the radio the service builds from it. It keeps nothing between
//! calls and reaches only `music.youtube.com`, through the host.
//!
//! `resolve`, which the providers table also gives this plugin, is not provided: the web client this plugin identifies
//! as gets stream URLs that cannot be used as given (section "Service" below, and ADR 0008, which lists the options
//! and waits for the user). The core's routing has no `resolve` provider until one is chosen.
//!
//! # How a call works
//!
//! - [`radio::continuation`] builds the requests ([`innertube`]), sends them through the `fetch` it is given, reads
//!   the answers ([`read`]) and picks the seed's song ([`matching`]). The tests give it a fake `fetch`; the Wasm build
//!   gives it the host's `http::fetch`.
//! - Song lengths and `Retry-After` delays are read by hand in [`length`], proved with Kani.
//!
//! # Service
//!
//! Checked on 2026-11-07 by calling the service from a home connection in France (a Mac), with the requests this
//! crate builds, one call each, without an account. `YouTube Music` answered every search with no result from a cloud
//! container whose address it placed in no country (`ZZ`); the plugin works only from where the service is served.
//!
//! - Host: `music.youtube.com`, over `https`.
//! - Client: `WEB_REMIX`, version `1.20261104.00.00`, the values the web site's page gives as `INNERTUBE_CLIENT_NAME`
//!   and `INNERTUBE_CLIENT_VERSION` that day. With version `1.20241127.01.00` every search answered no result.
//! - Headers: `Content-Type: application/json`, a desktop browser's `User-Agent`, `Origin` and `Referer` of the site,
//!   `Accept-Language: en`, `X-Youtube-Client-Name: 67` and `X-Youtube-Client-Version`: the headers the site's own
//!   requests carry. Which of them the service requires was not checked.
//! - `POST /youtubei/v1/search?prettyPrint=false` with `query` and the songs filter `params`
//!   `EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D`: `200`, `application/json`, 199 KB for "daft punk get lucky". Its songs are in
//!   `contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents[]`, in the
//!   section holding `musicShelfRenderer.contents[].musicResponsiveListItemRenderer` (20 that day): the video id in
//!   `playlistItemData.videoId`; the title in `flexColumns[0]`'s `text.runs`; in `flexColumns[1]`'s, the artists and
//!   the album as runs whose `navigationEndpoint.browseEndpoint` has a `browseId` and a page type
//!   `MUSIC_PAGE_TYPE_ARTIST` or `MUSIC_PAGE_TYPE_ALBUM`, separated by plain runs, and the length last (`6:10`); the
//!   thumbnails in `thumbnail.musicThumbnailRenderer.thumbnail.thumbnails[]` with `url`, `width` and `height`
//!   (60 and 120 pixels). A search with no result has no `musicShelfRenderer`, only an `itemSectionRenderer` with a
//!   `messageRenderer`.
//! - Searching an ISRC (`USQX91300108`) returned that recording first, followed by unrelated songs.
//! - `POST /youtubei/v1/next?prettyPrint=false` with `videoId`, `playlistId` `RDAMVM` + the video id and
//!   `isAudioOnly`: `200`, 583 KB. The queue is at
//!   `contents.singleColumnMusicWatchNextResultsRenderer.tabbedRenderer.watchNextTabbedResultsRenderer.tabs[0]
//!   .tabRenderer.content.musicQueueRenderer.content.playlistPanelRenderer.contents[]`: 50
//!   `playlistPanelVideoRenderer`, the first the seed itself, each with `videoId`, `title.runs`, `longBylineText.runs`
//!   (artists and album linked as in a search, then the year), `lengthText.runs` and `thumbnail.thumbnails[]`
//!   (60 to 544 pixels, on `yt3.googleusercontent.com`). For a video id the service does not know
//!   (`AAAAAAAAAAA`), the answer is `200` with a `musicQueueRenderer` that has no `content`.
//! - A body the service cannot read (`videoId` a number) is answered `400` with a JSON `error`.
//! - `POST /youtubei/v1/player?prettyPrint=false` with the client and a video id only: `200` with
//!   `playabilityStatus.status` `UNPLAYABLE`, reason "Video unavailable" (its error screen says "The page needs to be
//!   reloaded."), and no `streamingData`. No answer spoke of bot detection. The answer depends on a field that call
//!   did not send: with `playbackContext.contentPlaybackContext.signatureTimestamp` (`20760` that day, a number found
//!   in the player script the web page references, `/s/player/a5c2aafe/player_es6.vflset/en_US/base.js`, about
//!   2.66 MB, and not in the page itself), `racyCheckOk` and `contentCheckOk`, it is `200` with `OK` and
//!   `streamingData`. In that answer every audio format carries `signatureCipher` and no `url`, and the URL inside
//!   the cipher has an `n=` parameter and no `pot=`. On nine video ids, one call each: eight `OK`, in all eight every
//!   format (1 muxed, 16 to 18 adaptive) ciphered; one (`jNQXAC9IVRw`) `UNPLAYABLE` ("Video unavailable") even with
//!   the timestamp. The ids were mostly major-label clips; no small-artist upload was checked. Not checked: what
//!   `googlevideo.com` answers for a stream URL, how to use `serverAbrStreamingUrl`, and what the functions of the
//!   player script that put the signature back in order and transform `n` contain. Using these URLs needs those
//!   functions, which are JavaScript, so `resolve` is not provided (ADR 0008).
//! - Not checked: a `429`, and whether the service sends `Retry-After` with one (causing one means flooding the
//!   service; the plugin reads them as HTTP defines them); the service's limits on requests; any other status; its
//!   terms of use.

pub mod innertube;
pub mod length;
pub mod matching;
pub mod model;
pub mod radio;
pub mod read;

#[cfg(test)]
mod fixtures;

/// The guest side of the contract in `wit/`, generated by `wit-bindgen`.
///
/// The only module where `unsafe` is allowed, because the generated code needs it (ADR 0005). Nothing is written by
/// hand here but the macro calls. The exports are linked only for Wasm: natively, the tests call the plugin's
/// functions directly.
#[cfg_attr(
    target_arch = "wasm32",
    expect(
        unsafe_code,
        reason = "the exports wit-bindgen generates are `unsafe` (ADR 0005)"
    )
)]
#[expect(
    clippy::same_length_and_capacity,
    reason = "in the code wit-bindgen generates (ADR 0005)"
)]
mod bindings {
    wit_bindgen::generate!({ path: "../../wit", world: "vehicoule:plugin/radio-plugin@0.1.0" });

    #[cfg(target_arch = "wasm32")]
    use super::YtMusic;
    #[cfg(target_arch = "wasm32")]
    export!(YtMusic);
}

use bindings::exports::vehicoule::plugin::plugin::{Guest as PluginGuest, Manifest};
use bindings::vehicoule::plugin::types::{Capability, Domain};

/// The boundary between the contract's values and the plugin's own. Compiled for Wasm, where the export calls it, and
/// for the tests; natively nothing else would use it.
#[cfg(any(target_arch = "wasm32", test))]
mod contract;

/// The plugin, as the contract sees it.
struct YtMusic;

impl PluginGuest for YtMusic {
    fn describe() -> Manifest {
        Manifest {
            // The id is the crate's directory name and the name of the component the build writes
            // (docs/plugin-crates.md); the conformance test checks all three agree.
            id: String::from("ytmusic"),
            // The service's own name (AGENTS.md rule 16).
            name: String::from("YouTube Music"),
            version: String::from(env!("CARGO_PKG_VERSION")),
            domains: vec![Domain::Music],
            capabilities: vec![Capability::Radio],
            http_hosts: vec![String::from(innertube::HOST)],
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl bindings::exports::vehicoule::plugin::radio::Guest for YtMusic {
    fn continuation(
        seed: bindings::vehicoule::plugin::types::Track,
        exclude: Vec<String>,
        count: u32,
    ) -> Result<
        Vec<bindings::vehicoule::plugin::types::Track>,
        bindings::vehicoule::plugin::types::Error,
    > {
        contract::continuation(&seed, &exclude, count, |request| {
            contract::from_http(bindings::vehicoule::plugin::http::fetch(
                &contract::to_http(request),
            ))
        })
    }
}
