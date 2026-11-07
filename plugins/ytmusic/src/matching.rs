//! Finding, among the service's songs, the one that is the track a call looks for.
//!
//! The track may come from another plugin, so the match uses what every track has: the title, the artists, the
//! length, and the ISRC to search with when there is one (Vehicoule's plugin providers, section 2).

use crate::model::{Error, Song, Wanted};
use std::time::Duration;

/// How far a song's length may be from the track's and still be the same recording.
///
/// Not measured: chosen to absorb the service's lengths shown to the second (`6:10` for a recording another service
/// gives as 369 s) while telling apart the edits of one song, which differ by tens of seconds (`6:10` and `4:09` for
/// the album and radio versions of the song checked). To be measured on a sample of real tracks.
pub const LENGTH_TOLERANCE: Duration = Duration::from_secs(5);

/// The text to search the service with: the track's ISRC when it has one, else its title and first artist.
///
/// An ISRC is used as given only when it has the form of one (12 ASCII letters and digits, ISO 3901); the service
/// returned the recording first for one, on the one call checked.
///
/// # Errors
///
/// `invalid-input` for a track whose title has no letter or digit, which cannot be searched or matched.
///
/// ```
/// use vehicoule_plugin_ytmusic::matching::search_query;
/// use vehicoule_plugin_ytmusic::model::Wanted;
///
/// let mut track = Wanted::default();
/// track.title = String::from("Get Lucky");
/// track.artists = vec![String::from("Daft Punk")];
/// assert_eq!(search_query(&track).unwrap(), "Get Lucky Daft Punk");
/// track.isrc = Some(String::from("USQX91300108"));
/// assert_eq!(search_query(&track).unwrap(), "USQX91300108");
/// ```
pub fn search_query(wanted: &Wanted) -> Result<String, Error> {
    if normalize(&wanted.title).is_empty() {
        return Err(Error::InvalidInput(String::from(
            "the track's title has no letter or digit",
        )));
    }
    if let Some(isrc) = wanted.isrc.as_deref().filter(|isrc| is_isrc(isrc)) {
        return Ok(isrc.to_owned());
    }
    Ok(match wanted.artists.first() {
        Some(artist) => format!("{} {artist}", wanted.title),
        None => wanted.title.clone(),
    })
}

fn is_isrc(text: &str) -> bool {
    text.len() == 12 && text.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// The first song, in the service's order, that is the wanted track and was not rejected.
///
/// A song is the track when its title before any bracket or ` - ` is the track's, it shares an artist with it, and
/// their lengths, when both are known, are within [`LENGTH_TOLERANCE`]. Titles and names are compared in lower case,
/// letters and digits only.
#[must_use]
pub fn best_match<'a>(wanted: &Wanted, songs: &'a [Song], rejected: &[String]) -> Option<&'a Song> {
    let title = base_title(&wanted.title);
    let artists: Vec<String> = wanted
        .artists
        .iter()
        .map(|artist| normalize(artist))
        .collect();
    songs.iter().find(|song| {
        !rejected.contains(&song.id)
            && base_title(&song.title) == title
            && song
                .artists
                .iter()
                .any(|artist| artists.contains(&normalize(&artist.name)))
            && match (song.length, wanted.length) {
                (Some(song), Some(wanted)) => song.abs_diff(wanted) <= LENGTH_TOLERANCE,
                _ => true,
            }
    })
}

/// A title without what follows its first bracket or ` - `, where services put versions and featured artists:
/// `Get Lucky (feat. Pharrell Williams and Nile Rodgers)` is `get lucky`. A title that starts with one is kept whole.
fn base_title(title: &str) -> String {
    let end = [title.find('('), title.find('['), title.find(" - ")]
        .into_iter()
        .flatten()
        .min();
    let base = normalize(end.and_then(|end| title.get(..end)).unwrap_or(title));
    if base.is_empty() {
        normalize(title)
    } else {
        base
    }
}

/// Lower case, letters and digits only, one space between words.
fn normalize(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Link;

    fn wanted(title: &str, artists: &[&str], length: Option<u64>) -> Wanted {
        Wanted {
            title: title.into(),
            artists: artists.iter().map(|artist| String::from(*artist)).collect(),
            length: length.map(Duration::from_secs),
            isrc: None,
        }
    }

    fn song(id: &str, title: &str, artists: &[&str], length: Option<u64>) -> Song {
        Song {
            id: id.into(),
            title: title.into(),
            artists: artists
                .iter()
                .map(|name| Link {
                    id: format!("UC{name}"),
                    name: String::from(*name),
                })
                .collect(),
            length: length.map(Duration::from_secs),
            ..Song::default()
        }
    }

    #[test]
    fn searches_by_isrc_when_the_track_has_one() {
        let mut track = wanted("Get Lucky", &["Daft Punk", "Pharrell Williams"], None);
        assert_eq!(
            search_query(&track),
            Ok(String::from("Get Lucky Daft Punk"))
        );
        track.isrc = Some(String::from("USQX91300108"));
        assert_eq!(search_query(&track), Ok(String::from("USQX91300108")));
    }

    #[test]
    fn searches_by_title_when_the_isrc_is_not_one() {
        for isrc in [
            "",
            "USQX9130010",
            "USQX913001080",
            "USQX9130010-",
            "USQX 1300108",
            "USQX91300١٠",
        ] {
            let mut track = wanted("Get Lucky", &["Daft Punk"], None);
            track.isrc = Some(String::from(isrc));
            assert_eq!(
                search_query(&track),
                Ok(String::from("Get Lucky Daft Punk")),
                "{isrc:?}"
            );
        }
    }

    #[test]
    fn searches_by_title_alone_without_an_artist() {
        assert_eq!(
            search_query(&wanted("Get Lucky", &[], None)),
            Ok(String::from("Get Lucky"))
        );
    }

    #[test]
    fn refuses_a_title_with_nothing_to_search() {
        for title in ["", " ", "()", " - ", "!?"] {
            let mut track = wanted(title, &["Daft Punk"], None);
            track.isrc = Some(String::from("USQX91300108"));
            assert!(
                matches!(search_query(&track), Err(Error::InvalidInput(_))),
                "{title:?}"
            );
        }
    }

    #[test]
    fn matches_the_same_recording_across_services() {
        let track = wanted(
            "Get Lucky (feat. Pharrell Williams & Nile Rodgers)",
            &["Daft Punk"],
            Some(369),
        );
        let songs = [
            song(
                "radio",
                "Get Lucky (Radio Edit - feat. Pharrell Williams and Nile Rodgers)",
                &["Daft Punk"],
                Some(249),
            ),
            song(
                "album",
                "Get Lucky (feat. Pharrell Williams and Nile Rodgers)",
                &["DAFT PUNK", "Pharrell Williams"],
                Some(370),
            ),
        ];
        assert_eq!(
            best_match(&track, &songs, &[]).map(|song| song.id.as_str()),
            Some("album")
        );
    }

    #[test]
    fn keeps_the_services_order_among_matches() {
        let track = wanted("Get Lucky", &["Daft Punk"], None);
        let songs = [
            song("first", "Get Lucky", &["Daft Punk"], Some(370)),
            song("second", "Get Lucky", &["Daft Punk"], Some(370)),
        ];
        assert_eq!(
            best_match(&track, &songs, &[]).map(|song| song.id.as_str()),
            Some("first")
        );
    }

    #[test]
    fn never_returns_a_rejected_song() {
        let track = wanted("Get Lucky", &["Daft Punk"], None);
        let songs = [
            song("first", "Get Lucky", &["Daft Punk"], None),
            song("second", "Get Lucky", &["Daft Punk"], None),
        ];
        let rejected = [String::from("first")];
        assert_eq!(
            best_match(&track, &songs, &rejected).map(|song| song.id.as_str()),
            Some("second")
        );
        let rejected = [String::from("first"), String::from("second")];
        assert_eq!(best_match(&track, &songs, &rejected), None);
    }

    #[test]
    fn never_matches_another_title() {
        let track = wanted("Get Lucky", &["Daft Punk"], Some(370));
        for title in [
            "Get Luckier",
            "Lucky",
            "Get",
            "Instant Crush",
            "One More Time",
        ] {
            assert_eq!(
                best_match(&track, &[song("x", title, &["Daft Punk"], Some(370))], &[]),
                None,
                "{title:?}"
            );
        }
    }

    #[test]
    fn never_matches_another_artist() {
        let track = wanted("Get Lucky", &["Daft Punk"], Some(370));
        for artists in [
            &[][..],
            &["Funk Punk"][..],
            &["Daft", "Punk"][..],
            &["Daft Punk Tribute"][..],
        ] {
            assert_eq!(
                best_match(&track, &[song("x", "Get Lucky", artists, Some(370))], &[]),
                None,
                "{artists:?}"
            );
        }
        let without_artists = wanted("Get Lucky", &[], Some(370));
        assert_eq!(
            best_match(
                &without_artists,
                &[song("x", "Get Lucky", &["Daft Punk"], Some(370))],
                &[]
            ),
            None
        );
    }

    #[test]
    fn matches_lengths_within_the_tolerance_only() {
        let track = wanted("Get Lucky", &["Daft Punk"], Some(370));
        for (length, matches) in [
            (365, true),
            (370, true),
            (375, true),
            (364, false),
            (376, false),
            (249, false),
        ] {
            let songs = [song("x", "Get Lucky", &["Daft Punk"], Some(length))];
            assert_eq!(
                best_match(&track, &songs, &[]).is_some(),
                matches,
                "{length}"
            );
        }
    }

    #[test]
    fn an_unknown_length_does_not_prevent_a_match() {
        let songs = [song("x", "Get Lucky", &["Daft Punk"], None)];
        assert!(best_match(&wanted("Get Lucky", &["Daft Punk"], Some(370)), &songs, &[]).is_some());
        let songs = [song("x", "Get Lucky", &["Daft Punk"], Some(370))];
        assert!(best_match(&wanted("Get Lucky", &["Daft Punk"], None), &songs, &[]).is_some());
    }

    #[test]
    fn takes_the_title_before_a_bracket_or_a_dash() {
        assert_eq!(
            base_title("Get Lucky (feat. Pharrell Williams)"),
            "get lucky"
        );
        assert_eq!(base_title("Get Lucky [Remastered]"), "get lucky");
        assert_eq!(base_title("Get Lucky - Radio Edit"), "get lucky");
        assert_eq!(base_title("Get Lucky [Live] (2013)"), "get lucky");
        assert_eq!(base_title("Get Lucky (Live) [2013]"), "get lucky");
        assert_eq!(base_title("Get-Lucky"), "get lucky");
        assert_eq!(
            base_title("(I Can't Get No) Satisfaction"),
            "i can t get no satisfaction"
        );
        assert_eq!(base_title("Été"), "été");
    }

    #[test]
    fn normalizes_case_punctuation_and_spaces() {
        assert_eq!(normalize("  Pharrell   WILLIAMS!! "), "pharrell williams");
        assert_eq!(normalize("AC/DC"), "ac dc");
        assert_eq!(normalize("Beyoncé"), "beyoncé");
        assert_eq!(normalize("!?"), "");
    }
}
