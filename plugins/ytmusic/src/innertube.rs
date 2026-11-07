//! The requests the plugin sends to `YouTube Music`'s `InnerTube` API, and the status of what comes back.
//!
//! Every value here was checked against the service; the crate documentation's "Service" section says when and how.

use crate::length::parse_digits;
use crate::model::{Error, Request, Response};
use std::time::Duration;

/// The only host the plugin reaches.
pub const HOST: &str = "music.youtube.com";

/// The client the official web site identifies itself as, in the body and in `X-Youtube-Client-Name`.
const CLIENT_NAME: &str = "WEB_REMIX";
/// The number `X-Youtube-Client-Name` gives for that client.
const CLIENT_NAME_NUMBER: &str = "67";
/// The web site's version on 2026-11-07, read from its `INNERTUBE_CLIENT_VERSION`.
const CLIENT_VERSION: &str = "1.20261104.00.00";
/// The language of the texts the service writes; the plugin reads lengths, which are written the same in every one.
const LANGUAGE: &str = "en";
/// A desktop browser's `User-Agent`, as the web site's requests carry one.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
/// The search filter that keeps songs only, as the web site's "Songs" chip sends it.
const SONGS_FILTER: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D";
/// The prefix of the radio playlist the service builds from one song: `RDAMVM` and the song's video id.
const RADIO_PREFIX: &str = "RDAMVM";

/// The request that searches songs for a text.
///
/// ```
/// use vehicoule_plugin_ytmusic::innertube::search_request;
///
/// let request = search_request("USQX91300108");
/// assert_eq!(request.url, "https://music.youtube.com/youtubei/v1/search?prettyPrint=false");
/// ```
#[must_use]
pub fn search_request(query: &str) -> Request {
    let body = serde_json::json!({
        "context": context(),
        "query": query,
        "params": SONGS_FILTER,
    });
    request("search", &body)
}

/// The request that asks for the radio queue the service builds from a song.
///
/// ```
/// use vehicoule_plugin_ytmusic::innertube::next_request;
///
/// let request = next_request("4D7u5KF7SP8");
/// assert_eq!(request.url, "https://music.youtube.com/youtubei/v1/next?prettyPrint=false");
/// ```
#[must_use]
pub fn next_request(video_id: &str) -> Request {
    let body = serde_json::json!({
        "context": context(),
        "videoId": video_id,
        "playlistId": format!("{RADIO_PREFIX}{video_id}"),
        "isAudioOnly": true,
    });
    request("next", &body)
}

fn context() -> serde_json::Value {
    serde_json::json!({ "client": { "clientName": CLIENT_NAME, "clientVersion": CLIENT_VERSION, "hl": LANGUAGE } })
}

fn request(endpoint: &str, body: &serde_json::Value) -> Request {
    let origin = format!("https://{HOST}");
    let headers = [
        ("Content-Type", "application/json"),
        ("User-Agent", USER_AGENT),
        ("Origin", origin.as_str()),
        ("Referer", &format!("{origin}/")),
        ("Accept-Language", LANGUAGE),
        ("X-Youtube-Client-Name", CLIENT_NAME_NUMBER),
        ("X-Youtube-Client-Version", CLIENT_VERSION),
    ]
    .into_iter()
    .map(|(name, value)| (String::from(name), String::from(value)))
    .collect();
    Request {
        // `prettyPrint=false` drops the indentation, which the web site does not ask for either.
        url: format!("{origin}/youtubei/v1/{endpoint}?prettyPrint=false"),
        headers,
        // A `Value`'s `Display` writes JSON and cannot fail, where `to_vec` returns a `Result` never `Err` here.
        body: body.to_string().into_bytes(),
    }
}

/// Turns a status other than `200` into the contract's error.
///
/// `429` is `rate-limited`, with the delay of a `Retry-After` header given in seconds (RFC 9110, section 10.2.3; the
/// date form gives no delay). Every other status is `upstream`: the service gives `not-found` as content, never as a
/// status.
///
/// # Errors
///
/// The error for any status but `200`.
pub fn check_status(response: &Response) -> Result<(), Error> {
    match response.status {
        200 => Ok(()),
        429 => Err(Error::RateLimited(retry_after(&response.headers))),
        status => Err(Error::Upstream(format!(
            "the service answered status {status}"
        ))),
    }
}

fn retry_after(headers: &[(String, String)]) -> Option<Duration> {
    let (_, value) = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))?;
    parse_digits(value.trim().as_bytes()).map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
        request
            .headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn body(request: &Request) -> Value {
        serde_json::from_slice(&request.body).unwrap()
    }

    fn sends_the_web_sites_headers(request: &Request) {
        assert_eq!(request.headers.len(), 7);
        assert_eq!(header(request, "Content-Type"), Some("application/json"));
        assert_eq!(header(request, "User-Agent"), Some(USER_AGENT));
        assert_eq!(header(request, "Origin"), Some("https://music.youtube.com"));
        assert_eq!(
            header(request, "Referer"),
            Some("https://music.youtube.com/")
        );
        assert_eq!(header(request, "Accept-Language"), Some("en"));
        assert_eq!(header(request, "X-Youtube-Client-Name"), Some("67"));
        assert_eq!(
            header(request, "X-Youtube-Client-Version"),
            Some("1.20261104.00.00")
        );
    }

    #[test]
    fn searches_songs_with_the_web_client() {
        let request = search_request("daft punk \"get lucky\"");
        assert_eq!(
            request.url,
            "https://music.youtube.com/youtubei/v1/search?prettyPrint=false"
        );
        sends_the_web_sites_headers(&request);
        assert_eq!(
            body(&request),
            json!({
                "context": { "client": { "clientName": "WEB_REMIX", "clientVersion": "1.20261104.00.00", "hl": "en" } },
                "query": "daft punk \"get lucky\"",
                "params": "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D",
            })
        );
    }

    #[test]
    fn asks_for_the_radio_of_a_song() {
        let request = next_request("4D7u5KF7SP8");
        assert_eq!(
            request.url,
            "https://music.youtube.com/youtubei/v1/next?prettyPrint=false"
        );
        sends_the_web_sites_headers(&request);
        assert_eq!(
            body(&request),
            json!({
                "context": { "client": { "clientName": "WEB_REMIX", "clientVersion": "1.20261104.00.00", "hl": "en" } },
                "videoId": "4D7u5KF7SP8",
                "playlistId": "RDAMVM4D7u5KF7SP8",
                "isAudioOnly": true,
            })
        );
    }

    fn response(status: u16, headers: &[(&str, &str)]) -> Response {
        Response::new(
            status,
            headers
                .iter()
                .map(|(name, value)| (String::from(*name), String::from(*value)))
                .collect(),
            Vec::new(),
        )
    }

    #[test]
    fn only_200_is_success() {
        assert_eq!(check_status(&response(200, &[])), Ok(()));
        for status in [
            0, 199, 201, 204, 301, 304, 400, 403, 404, 500, 503, 599, 65535,
        ] {
            assert_eq!(
                check_status(&response(status, &[])),
                Err(Error::Upstream(format!(
                    "the service answered status {status}"
                ))),
            );
        }
    }

    #[test]
    fn rate_limited_carries_the_services_delay() {
        assert_eq!(
            check_status(&response(429, &[("Retry-After", "120")])),
            Err(Error::RateLimited(Some(Duration::from_secs(120)))),
        );
        assert_eq!(
            check_status(&response(429, &[("x-other", "1"), ("retry-after", " 7 ")])),
            Err(Error::RateLimited(Some(Duration::from_secs(7)))),
        );
    }

    #[test]
    fn rate_limited_without_a_delay_in_seconds_has_none() {
        for headers in [
            &[][..],
            &[("Retry-After", "Wed, 21 Oct 2026 07:28:00 GMT")][..],
            &[("Retry-After", "")][..],
            &[("Retry-After", "-1")][..],
            &[("Retry-After", "99999999999999999999")][..],
            &[("Retry-After-Not", "5")][..],
        ] {
            assert_eq!(
                check_status(&response(429, headers)),
                Err(Error::RateLimited(None)),
                "{headers:?}"
            );
        }
    }
}
